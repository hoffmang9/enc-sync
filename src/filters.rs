use std::collections::HashSet;

use crate::catalog::Cell;
use crate::config::Config;

#[derive(Debug, Clone)]
pub struct Filters {
    states: HashSet<String>,
    regions: HashSet<String>,
    coast_guard_districts: HashSet<String>,
}

impl Filters {
    pub fn from_config(config: &Config) -> Self {
        Self {
            states: normalize_codes(&config.states),
            regions: normalize_codes(&config.regions),
            coast_guard_districts: normalize_codes(&config.coast_guard_districts),
        }
    }

    pub fn is_active(&self) -> bool {
        !self.states.is_empty()
            || !self.regions.is_empty()
            || !self.coast_guard_districts.is_empty()
    }

    pub fn active_counts(&self) -> (usize, usize, usize) {
        (
            self.states.len(),
            self.regions.len(),
            self.coast_guard_districts.len(),
        )
    }

    pub fn matches(&self, cell: &Cell) -> bool {
        if !self.is_active() {
            return true;
        }
        dimension_matches(&self.states, &cell.states)
            || dimension_matches(&self.regions, &cell.regions)
            || dimension_matches(&self.coast_guard_districts, &cell.coast_guard_districts)
    }
}

fn dimension_matches(filter: &HashSet<String>, values: &[String]) -> bool {
    if filter.is_empty() {
        return false;
    }
    values
        .iter()
        .any(|value| filter.contains(&value.trim().to_ascii_uppercase()))
}

fn normalize_codes(values: &[String]) -> HashSet<String> {
    values
        .iter()
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| !value.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Cell;
    use crate::config::{default_catalog_url, Config};
    use std::path::PathBuf;

    fn sample_cell(name: &str, states: &[&str], regions: &[&str], cgd: &[&str]) -> Cell {
        Cell {
            name: name.to_string(),
            url: format!("https://example.test/{name}.zip"),
            timestamp: 1_700_000_000,
            states: states.iter().map(|s| (*s).to_string()).collect(),
            regions: regions.iter().map(|r| (*r).to_string()).collect(),
            coast_guard_districts: cgd.iter().map(|d| (*d).to_string()).collect(),
        }
    }

    fn test_config(states: Vec<&str>, regions: Vec<&str>, cgd: Vec<&str>) -> Config {
        Config {
            chart_dir: PathBuf::from("/tmp/charts"),
            catalog_url: default_catalog_url(),
            states: states.into_iter().map(str::to_string).collect(),
            regions: regions.into_iter().map(str::to_string).collect(),
            coast_guard_districts: cgd.into_iter().map(str::to_string).collect(),
            restart_opencpn: true,
            rebuild_chart_db: true,
        }
    }

    #[test]
    fn normalize_codes_trims_and_uppercases() {
        let codes = normalize_codes(&[
            " ca ".to_string(),
            String::new(),
            "  ".to_string(),
            "or".to_string(),
        ]);
        assert_eq!(codes, HashSet::from(["CA".to_string(), "OR".to_string()]));
    }

    #[test]
    fn inactive_filters_match_every_cell() {
        let filters = Filters::from_config(&test_config(vec![], vec![], vec![]));
        assert!(!filters.is_active());
        let cell = sample_cell("US5FL01M", &["FL"], &[], &[]);
        assert!(filters.matches(&cell));
    }

    #[test]
    fn filter_matches_state_or_region() {
        let filters = Filters::from_config(&test_config(vec!["CA"], vec![], vec![]));
        let cell = sample_cell("US5CA01M", &["CA"], &[], &[]);
        assert!(filters.matches(&cell));

        let other = Cell {
            states: vec!["FL".to_string()],
            ..cell.clone()
        };
        assert!(!filters.matches(&other));
    }

    #[test]
    fn filter_matches_region_when_state_does_not() {
        let filters = Filters::from_config(&test_config(vec![], vec!["14"], vec![]));
        let cell = sample_cell("US5WA01M", &["WA"], &["14"], &[]);
        assert!(filters.matches(&cell));

        let other = sample_cell("US5FL01M", &["FL"], &["5"], &[]);
        assert!(!filters.matches(&other));
    }

    #[test]
    fn filter_matches_coast_guard_district() {
        let filters = Filters::from_config(&test_config(vec![], vec![], vec!["11"]));
        let cell = sample_cell("US5CA01M", &[], &[], &["11"]);
        assert!(filters.matches(&cell));

        let other = sample_cell("US5FL01M", &[], &[], &["7"]);
        assert!(!filters.matches(&other));
    }

    #[test]
    fn filter_state_matching_is_case_insensitive() {
        let filters = Filters::from_config(&test_config(vec!["CA"], vec![], vec![]));
        let cell = sample_cell("US5CA01M", &["ca"], &[], &[]);
        assert!(filters.matches(&cell));
    }

    #[test]
    fn filters_from_config_normalizes_codes() {
        let config = Config {
            chart_dir: PathBuf::from("/tmp/charts"),
            catalog_url: default_catalog_url(),
            states: vec![" ca ".to_string(), "".to_string()],
            regions: vec!["14".to_string()],
            coast_guard_districts: vec![],
            restart_opencpn: true,
            rebuild_chart_db: true,
        };
        let filters = Filters::from_config(&config);
        assert!(filters.matches(&sample_cell("US5CA01M", &["CA"], &[], &[])));
        assert!(filters.matches(&sample_cell("US5WA01M", &["WA"], &["14"], &[])));
        assert!(!filters.matches(&sample_cell("US5OR01M", &["OR"], &[], &[])));
    }

    #[test]
    fn active_counts_reflects_configured_filters() {
        let filters = Filters::from_config(&test_config(vec!["CA", "OR"], vec!["14"], vec!["11"]));
        assert_eq!(filters.active_counts(), (2, 1, 1));
        assert!(filters.is_active());
    }
}
