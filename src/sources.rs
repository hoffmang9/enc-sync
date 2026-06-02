use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::source_norm::normalize_numeric_code;

include!(concat!(env!("OUT_DIR"), "/sources_generated.rs"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceKind {
    All,
    State(&'static str),
    Region(&'static str),
    CoastGuardDistrict(&'static str),
    InlandMain,
    InlandBuoys,
    InlandOverlays,
}

#[derive(Debug, Clone)]
pub struct ChartSource {
    pub name: &'static str,
    pub catalog_url: &'static str,
    pub catalog_filename: &'static str,
    /// Folder name under `ENC/`, e.g. `US_CA`.
    pub folder: &'static str,
    pub kind: SourceKind,
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
            return format!(
                "{}/{}",
                base.trim_end_matches('/'),
                self.catalog_filename
            );
        }
        self.catalog_url.to_string()
    }
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

    fn configured(&self, source: &ChartSource) -> bool {
        match source.kind {
            SourceKind::All => self.all_enc,
            SourceKind::State(code) => self.states.contains(code),
            SourceKind::Region(code) => self.regions.contains(code),
            SourceKind::CoastGuardDistrict(code) => self.coast_guard_districts.contains(code),
            SourceKind::InlandMain | SourceKind::InlandBuoys | SourceKind::InlandOverlays => {
                self.inland
            }
        }
    }
}

pub fn enc_root(config: &Config) -> PathBuf {
    let base = &config.chart_dir;
    if base.file_name().is_some_and(|name| name.eq_ignore_ascii_case("ENC")) {
        return base.clone();
    }
    base.join("ENC")
}

pub fn all_chart_sources() -> &'static [ChartSource] {
    EMBEDDED_SOURCES
}

pub fn select_sources(config: &Config) -> Result<Vec<ChartSource>> {
    let selection = SourceSelection::from_config(config);
    let enc_root = enc_root(config);
    let known = all_chart_sources();
    let known_by_folder: HashMap<_, _> = known.iter().map(|s| (s.folder, s)).collect();

    let mut folders: HashSet<&'static str> = HashSet::new();
    for source in known.iter().filter(|source| selection.configured(source)) {
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

    let mut selected: Vec<ChartSource> = folders
        .into_iter()
        .map(|folder| (*known_by_folder[folder]).clone())
        .collect();
    selected.sort_by(|a, b| a.folder.cmp(b.folder));
    Ok(selected)
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
    use tempfile::TempDir;

    #[test]
    fn embedded_sources_include_wa_or_ca_and_inland() {
        let sources = all_chart_sources();
        assert_eq!(sources.len(), 68);
        assert!(sources.iter().any(|s| s.folder == "US_CA"));
        assert!(sources.iter().any(|s| s.folder == "US_OR"));
        assert!(sources.iter().any(|s| s.folder == "US_WA"));
        assert!(sources.iter().any(|s| s.folder == "US_INLAND"));
        assert!(sources.iter().any(|s| s.folder == "US"));
    }

    #[test]
    fn config_selects_minimum_state_folders() {
        let config = Config {
            chart_dir: PathBuf::from("/charts"),
            states: vec!["CA".into(), "WA".into()],
            regions: vec![],
            coast_guard_districts: vec![],
            all_enc: false,
            inland: false,
            catalog_base_url: None,
            restart_opencpn: false,
            rebuild_chart_db: false,
        };
        let selected = select_sources(&config).unwrap();
        let folders: Vec<_> = selected.iter().map(|s| s.folder).collect();
        assert_eq!(folders, vec!["US_CA", "US_WA"]);
    }

    #[test]
    fn selection_matches_state_region_and_cgd() {
        let config = Config {
            chart_dir: PathBuf::from("/charts"),
            states: vec!["CA".into()],
            regions: vec!["14".into()],
            coast_guard_districts: vec!["11".into()],
            all_enc: false,
            inland: false,
            catalog_base_url: None,
            restart_opencpn: false,
            rebuild_chart_db: false,
        };
        let selected = select_sources(&config).unwrap();
        let folders: Vec<_> = selected.iter().map(|s| s.folder).collect();
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

        let config = Config {
            chart_dir: base.path().to_path_buf(),
            states: vec!["CA".into()],
            regions: vec![],
            coast_guard_districts: vec![],
            all_enc: false,
            inland: false,
            catalog_base_url: None,
            restart_opencpn: false,
            rebuild_chart_db: false,
        };
        let selected = select_sources(&config).unwrap();
        let folders: Vec<_> = selected.iter().map(|s| s.folder).collect();
        assert_eq!(folders, vec!["US_CA", "US_OR"]);
    }

    #[test]
    fn discovery_uses_existing_enc_folders_when_unconfigured() {
        let base = TempDir::new().unwrap();
        let enc = base.path().join("ENC");
        fs::create_dir_all(enc.join("US_CA")).unwrap();
        fs::create_dir_all(enc.join("US_OR")).unwrap();
        fs::create_dir_all(enc.join("ignored")).unwrap();

        let config = Config {
            chart_dir: base.path().to_path_buf(),
            states: vec![],
            regions: vec![],
            coast_guard_districts: vec![],
            all_enc: false,
            inland: false,
            catalog_base_url: None,
            restart_opencpn: false,
            rebuild_chart_db: false,
        };
        let selected = select_sources(&config).unwrap();
        let folders: Vec<_> = selected.iter().map(|s| s.folder).collect();
        assert_eq!(folders, vec!["US_CA", "US_OR"]);
    }

}
