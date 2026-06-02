use std::collections::HashSet;

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
            regions: config
                .regions
                .iter()
                .map(|value| normalize_numeric_code(value))
                .filter(|value| !value.is_empty())
                .collect(),
            coast_guard_districts: config
                .coast_guard_districts
                .iter()
                .map(|value| normalize_numeric_code(value))
                .filter(|value| !value.is_empty())
                .collect(),
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

    pub fn matches_state(&self, state: &str) -> bool {
        self.states.contains(&state.trim().to_ascii_uppercase())
    }

    pub fn matches_region(&self, region: &str) -> bool {
        self.regions.contains(&normalize_numeric_code(region))
    }

    pub fn matches_cgd(&self, district: &str) -> bool {
        self.coast_guard_districts
            .contains(&normalize_numeric_code(district))
    }
}

fn normalize_codes(values: &[String]) -> HashSet<String> {
    values
        .iter()
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| !value.is_empty())
        .collect()
}

fn normalize_numeric_code(raw: &str) -> String {
    let trimmed = raw.trim().to_ascii_uppercase();
    let stripped = trimmed.trim_start_matches('0');
    if stripped.is_empty() {
        "0".to_string()
    } else {
        stripped.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::path::PathBuf;

    fn test_config(states: Vec<&str>, regions: Vec<&str>, cgd: Vec<&str>) -> Config {
        Config {
            chart_dir: PathBuf::from("/tmp/charts"),
            states: states.into_iter().map(str::to_string).collect(),
            regions: regions.into_iter().map(str::to_string).collect(),
            coast_guard_districts: cgd.into_iter().map(str::to_string).collect(),
            all_enc: false,
            inland: false,
            catalog_base_url: None,
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
    fn filter_matches_state_or_region_or_cgd() {
        let filters = Filters::from_config(&test_config(vec!["CA"], vec![], vec![]));
        assert!(filters.matches_state("CA"));
        assert!(!filters.matches_state("FL"));

        let region_filters = Filters::from_config(&test_config(vec![], vec!["14"], vec![]));
        assert!(region_filters.matches_region("14"));

        let cgd_filters = Filters::from_config(&test_config(vec![], vec![], vec!["11"]));
        assert!(cgd_filters.matches_cgd("11"));
    }

    #[test]
    fn active_counts_reflects_configured_filters() {
        let filters = Filters::from_config(&test_config(vec!["CA", "OR"], vec!["14"], vec!["11"]));
        assert_eq!(filters.active_counts(), (2, 1, 1));
        assert!(filters.is_active());
    }
}
