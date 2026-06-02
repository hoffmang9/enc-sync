use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::source_norm::normalize_numeric_code;
use crate::source_taxonomy::{folder_matches_selection, SelectionCriteria};

include!(concat!(env!("OUT_DIR"), "/sources_generated.rs"));

#[derive(Debug, Clone)]
pub struct ChartSource {
    pub name: &'static str,
    pub catalog_url: &'static str,
    pub catalog_filename: &'static str,
    /// Folder name under `ENC/`, e.g. `US_CA`.
    pub folder: &'static str,
}

impl ChartSource {
    pub fn chart_dir(&self, enc_root: &Path) -> PathBuf {
        enc_root.join(self.folder)
    }

    pub fn resolve_catalog_url(&self, config: &Config) -> String {
        if let Some(base) = config
            .catalog_base_url
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            return format!("{}/{}", base.trim_end_matches('/'), self.catalog_filename);
        }
        self.catalog_url.to_string()
    }
}

#[derive(Debug, Clone)]
pub struct SelectedSources {
    pub sources: Vec<&'static ChartSource>,
    pub minimum_summary: Option<String>,
}

#[derive(Debug, Clone)]
struct SourceSelection {
    states: HashSet<String>,
    regions: HashSet<String>,
    coast_guard_districts: HashSet<String>,
    all_enc: bool,
    inland: bool,
}

impl SourceSelection {
    fn from_config(config: &Config) -> Self {
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
            all_enc: config.all_enc,
            inland: config.inland,
        }
    }

    fn criteria(&self) -> SelectionCriteria<'_> {
        SelectionCriteria {
            states: &self.states,
            regions: &self.regions,
            coast_guard_districts: &self.coast_guard_districts,
            all_enc: self.all_enc,
            inland: self.inland,
        }
    }

    fn configured(&self, folder: &str) -> bool {
        folder_matches_selection(folder, &self.criteria())
    }

    fn summary(&self) -> Option<String> {
        let mut parts = Vec::new();
        if !self.states.is_empty() {
            parts.push(format!("states [{}]", join_sorted(&self.states)));
        }
        if !self.regions.is_empty() {
            parts.push(format!("regions [{}]", join_sorted(&self.regions)));
        }
        if !self.coast_guard_districts.is_empty() {
            parts.push(format!(
                "CG districts [{}]",
                join_sorted(&self.coast_guard_districts)
            ));
        }
        if self.all_enc {
            parts.push("ENC/US".to_string());
        }
        if self.inland {
            parts.push("US Army Corps inland".to_string());
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("; "))
        }
    }
}

pub fn enc_root(config: &Config) -> PathBuf {
    let base = &config.chart_dir;
    if base
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("ENC"))
    {
        return base.clone();
    }
    base.join("ENC")
}

pub fn all_chart_sources() -> &'static [ChartSource] {
    EMBEDDED_SOURCES
}

pub fn select_sources(config: &Config) -> Result<SelectedSources> {
    let selection = SourceSelection::from_config(config);
    let minimum_summary = selection.summary();
    let enc_root = enc_root(config);
    let known = all_chart_sources();
    let known_by_folder: HashMap<_, _> = known.iter().map(|s| (s.folder, s)).collect();

    let mut folders: HashSet<&'static str> = HashSet::new();
    for source in known
        .iter()
        .filter(|source| selection.configured(source.folder))
    {
        folders.insert(source.folder);
    }

    if enc_root.is_dir() {
        for entry in fs::read_dir(&enc_root)
            .with_context(|| format!("reading ENC directory {}", enc_root.display()))?
        {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let folder = entry.file_name().to_string_lossy().into_owned();
            if let Some(source) = known_by_folder.get(folder.as_str()) {
                folders.insert(source.folder);
            } else {
                log::debug!("ignoring unrecognized ENC folder {}", folder);
            }
        }
    }

    if folders.is_empty() {
        bail!(
            "no chart sources to sync; configure states/regions/CGD (or all_enc/inland) \
             and/or add recognized OpenCPN ENC folders under {}",
            enc_root.display()
        );
    }

    let mut sources: Vec<&'static ChartSource> = folders
        .into_iter()
        .map(|folder| known_by_folder[folder])
        .collect();
    sources.sort_by_key(|source| source.folder);
    Ok(SelectedSources {
        sources,
        minimum_summary,
    })
}

fn normalize_codes(values: &[String]) -> HashSet<String> {
    values
        .iter()
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| !value.is_empty())
        .collect()
}

fn join_sorted(values: &HashSet<String>) -> String {
    let mut sorted: Vec<_> = values.iter().cloned().collect();
    sorted.sort();
    sorted.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config(chart_dir: PathBuf) -> Config {
        Config {
            chart_dir,
            states: vec![],
            regions: vec![],
            coast_guard_districts: vec![],
            all_enc: false,
            inland: false,
            catalog_base_url: None,
            restart_opencpn: false,
            rebuild_chart_db: false,
        }
    }

    #[test]
    fn embedded_sources_include_wa_or_ca_and_inland() {
        let sources = all_chart_sources();
        assert!(!sources.is_empty());
        assert!(sources.iter().any(|s| s.folder == "US_CA"));
        assert!(sources.iter().any(|s| s.folder == "US_OR"));
        assert!(sources.iter().any(|s| s.folder == "US_WA"));
        assert!(sources.iter().any(|s| s.folder == "US_INLAND"));
        assert!(sources.iter().any(|s| s.folder == "US"));
    }

    #[test]
    fn config_minimum_summary_uses_normalized_codes() {
        let mut config = test_config(PathBuf::from("/charts"));
        config.regions = vec!["01".into()];
        config.coast_guard_districts = vec!["11".into()];
        let summary = select_sources(&config).unwrap().minimum_summary.unwrap();
        assert!(summary.contains("regions [1]"));
        assert!(summary.contains("CG districts [11]"));
    }

    #[test]
    fn config_selects_minimum_state_folders() {
        let mut config = test_config(PathBuf::from("/charts"));
        config.states = vec!["CA".into(), "WA".into()];
        let selected = select_sources(&config).unwrap();
        let folders: Vec<_> = selected.sources.iter().map(|s| s.folder).collect();
        assert_eq!(folders, vec!["US_CA", "US_WA"]);
    }

    #[test]
    fn selection_matches_state_region_and_cgd() {
        let mut config = test_config(PathBuf::from("/charts"));
        config.states = vec!["CA".into()];
        config.regions = vec!["14".into()];
        config.coast_guard_districts = vec!["11".into()];
        let selected = select_sources(&config).unwrap();
        let folders: Vec<_> = selected.sources.iter().map(|s| s.folder).collect();
        assert!(folders.contains(&"US_CA"));
        assert!(folders.contains(&"US_REGION14"));
        assert!(folders.contains(&"US_CGD11"));
    }

    #[test]
    fn config_minimum_plus_discovered_folders() {
        let base = TempDir::new().unwrap();
        let enc = base.path().join("ENC");
        fs::create_dir_all(enc.join("US_CA")).unwrap();
        fs::create_dir_all(enc.join("US_OR")).unwrap();

        let mut config = test_config(base.path().to_path_buf());
        config.states = vec!["CA".into()];
        let selected = select_sources(&config).unwrap();
        let folders: Vec<_> = selected.sources.iter().map(|s| s.folder).collect();
        assert_eq!(folders, vec!["US_CA", "US_OR"]);
    }

    #[test]
    fn discovery_uses_existing_enc_folders_when_unconfigured() {
        let base = TempDir::new().unwrap();
        let enc = base.path().join("ENC");
        fs::create_dir_all(enc.join("US_CA")).unwrap();
        fs::create_dir_all(enc.join("US_OR")).unwrap();
        fs::create_dir_all(enc.join("ignored")).unwrap();

        let config = test_config(base.path().to_path_buf());
        let selected = select_sources(&config).unwrap();
        let folders: Vec<_> = selected.sources.iter().map(|s| s.folder).collect();
        assert_eq!(folders, vec!["US_CA", "US_OR"]);
    }
}
