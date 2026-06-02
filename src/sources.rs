use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{bail, Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::config::Config;
use crate::filters::Filters;

const OPENCPN_SOURCES_XML: &str = include_str!("../data/opencpn_enc_sources.xml");
const USERDATA_PREFIX: &str = "{USERDATA}/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceKind {
    All,
    State(String),
    Region(String),
    CoastGuardDistrict(String),
    InlandMain,
    InlandBuoys,
    InlandOverlays,
}

#[derive(Debug, Clone)]
pub struct ChartSource {
    pub name: String,
    pub catalog_url: String,
    pub catalog_filename: String,
    /// Folder name under `ENC/`, e.g. `US_CA`.
    pub folder: String,
    pub kind: SourceKind,
}

impl ChartSource {
    pub fn chart_dir(&self, enc_root: &Path) -> PathBuf {
        enc_root.join(&self.folder)
    }
}

pub fn enc_root(config: &Config) -> PathBuf {
    let base = &config.chart_dir;
    if base.file_name().is_some_and(|name| name.eq_ignore_ascii_case("ENC")) {
        return base.clone();
    }
    base.join("ENC")
}

pub fn catalog_filename(url: &str) -> &str {
    url.rsplit('/')
        .next()
        .filter(|part| !part.is_empty())
        .unwrap_or("catalog.xml")
}

pub fn all_chart_sources() -> &'static [ChartSource] {
    static SOURCES: OnceLock<Vec<ChartSource>> = OnceLock::new();
    SOURCES
        .get_or_init(parse_opencpn_sources)
        .as_slice()
}

pub fn select_sources(config: &Config, filters: &Filters) -> Result<Vec<ChartSource>> {
    let enc_root = enc_root(config);
    let known = all_chart_sources();
    let known_by_folder: HashMap<_, _> = known.iter().map(|s| (s.folder.as_str(), s)).collect();

    if filters.is_active() || config.all_enc || config.inland {
        let selected: Vec<ChartSource> = known
            .iter()
            .filter(|source| source_enabled(source, config, filters))
            .cloned()
            .collect();
        if selected.is_empty() {
            bail!("no chart sources matched the current filters and options");
        }
        return Ok(selected);
    }

    if !enc_root.is_dir() {
        bail!(
            "ENC directory {} does not exist; create OpenCPN chart folders or configure filters",
            enc_root.display()
        );
    }

    let mut selected = Vec::new();
    for entry in fs::read_dir(&enc_root)
        .with_context(|| format!("reading ENC directory {}", enc_root.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let folder = entry.file_name().to_string_lossy().into_owned();
        if let Some(source) = known_by_folder.get(folder.as_str()) {
            selected.push((*source).clone());
        } else {
            log::debug!("ignoring unrecognized ENC folder {}", folder);
        }
    }

    selected.sort_by(|a, b| a.folder.cmp(&b.folder));
    if selected.is_empty() {
        bail!(
            "no recognized OpenCPN ENC folders under {}; add chart sources in OpenCPN or configure filters",
            enc_root.display()
        );
    }
    Ok(selected)
}

fn source_enabled(source: &ChartSource, config: &Config, filters: &Filters) -> bool {
    match &source.kind {
        SourceKind::All => config.all_enc,
        SourceKind::State(code) => filters.matches_state(code),
        SourceKind::Region(code) => filters.matches_region(code),
        SourceKind::CoastGuardDistrict(code) => filters.matches_cgd(code),
        SourceKind::InlandMain | SourceKind::InlandBuoys | SourceKind::InlandOverlays => {
            config.inland
        }
    }
}

fn parse_opencpn_sources() -> Vec<ChartSource> {
    let mut reader = Reader::from_str(OPENCPN_SOURCES_XML);
    reader.config_mut().trim_text(true);

    let mut sources = Vec::new();
    let mut buf = Vec::new();
    let mut in_catalog = false;
    let mut current_field = None::<String>;
    let mut current_text = String::new();
    let mut current_name = None::<String>;
    let mut current_location = None::<String>;
    let mut current_dir = None::<String>;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(tag)) => {
                let field = String::from_utf8_lossy(tag.name().as_ref()).into_owned();
                if field == "catalog" {
                    in_catalog = true;
                    current_name = None;
                    current_location = None;
                    current_dir = None;
                } else if in_catalog {
                    current_field = Some(field);
                    current_text.clear();
                }
            }
            Ok(Event::Text(text)) if in_catalog && current_field.is_some() => {
                current_text.push_str(&text.unescape().unwrap_or_default());
            }
            Ok(Event::End(tag)) => {
                let field = String::from_utf8_lossy(tag.name().as_ref()).into_owned();
                if field == "catalog" {
                    if let (Some(name), Some(location), Some(dir)) =
                        (current_name.take(), current_location.take(), current_dir.take())
                    {
                        if let Some(source) = build_source(name, location, dir) {
                            sources.push(source);
                        }
                    }
                    in_catalog = false;
                } else if in_catalog {
                    let value = std::mem::take(&mut current_text).trim().to_string();
                    current_field = None;
                    if value.is_empty() {
                        continue;
                    }
                    match field.as_str() {
                        "name" => current_name = Some(value),
                        "location" => current_location = Some(value),
                        "dir" => current_dir = Some(value),
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                log::warn!("failed to parse embedded OpenCPN sources XML: {error}");
                break;
            }
        }
        buf.clear();
    }

    if sources.is_empty() {
        sources.extend(fallback_sources());
    } else {
        sources.sort_by(|a, b| a.folder.cmp(&b.folder));
    }
    sources
}

fn build_source(name: String, location: String, dir: String) -> Option<ChartSource> {
    let rel = dir.strip_prefix(USERDATA_PREFIX)?.trim_start_matches('/');
    let folder = rel.rsplit('/').next()?.to_string();
    let kind = classify_folder(&folder)?;
    Some(ChartSource {
        catalog_filename: catalog_filename(&location).to_string(),
        name,
        catalog_url: location,
        folder,
        kind,
    })
}

pub fn classify_folder(folder: &str) -> Option<SourceKind> {
    if folder == "US" {
        return Some(SourceKind::All);
    }
    if folder == "US_INLAND" {
        return Some(SourceKind::InlandMain);
    }
    if folder == "US_INLAND_BUOYS" {
        return Some(SourceKind::InlandBuoys);
    }
    if folder == "US_INLAND_OVERLAYS" {
        return Some(SourceKind::InlandOverlays);
    }
    if let Some(code) = folder.strip_prefix("US_CGD") {
        return Some(SourceKind::CoastGuardDistrict(normalize_numeric_code(code)));
    }
    if let Some(code) = folder.strip_prefix("US_REGION") {
        return Some(SourceKind::Region(normalize_numeric_code(code)));
    }
    if let Some(code) = folder.strip_prefix("US_") {
        return Some(SourceKind::State(code.to_ascii_uppercase()));
    }
    None
}

fn normalize_numeric_code(raw: &str) -> String {
    raw.trim_start_matches('0').to_string()
}

fn fallback_sources() -> Vec<ChartSource> {
    vec![
        make_source(
            "All",
            "https://www.charts.noaa.gov/ENCs/ENCProdCat.xml",
            "US",
            SourceKind::All,
        ),
        make_source(
            "CA - California",
            "https://www.charts.noaa.gov/ENCs/CA_ENCProdCat.xml",
            "US_CA",
            SourceKind::State("CA".into()),
        ),
        make_source(
            "OR - Oregon",
            "https://www.charts.noaa.gov/ENCs/OR_ENCProdCat.xml",
            "US_OR",
            SourceKind::State("OR".into()),
        ),
        make_source(
            "WA - Washington",
            "https://www.charts.noaa.gov/ENCs/WA_ENCProdCat.xml",
            "US_WA",
            SourceKind::State("WA".into()),
        ),
    ]
}

fn make_source(name: &str, url: &str, folder: &str, kind: SourceKind) -> ChartSource {
    ChartSource {
        name: name.to_string(),
        catalog_url: url.to_string(),
        catalog_filename: catalog_filename(url).to_string(),
        folder: folder.to_string(),
        kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::filters::Filters;
    use tempfile::TempDir;

    #[test]
    fn embedded_sources_include_wa_or_ca_and_inland() {
        let sources = all_chart_sources();
        assert!(sources.iter().any(|s| s.folder == "US_CA"));
        assert!(sources.iter().any(|s| s.folder == "US_OR"));
        assert!(sources.iter().any(|s| s.folder == "US_WA"));
        assert!(sources.iter().any(|s| s.folder == "US_INLAND"));
        assert!(sources.iter().any(|s| s.folder == "US"));
    }

    #[test]
    fn filters_select_state_folders() {
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
        let filters = Filters::from_config(&config);
        let selected = select_sources(&config, &filters).unwrap();
        let folders: Vec<_> = selected.iter().map(|s| s.folder.as_str()).collect();
        assert_eq!(folders, vec!["US_CA", "US_WA"]);
    }

    #[test]
    fn discovery_uses_existing_enc_folders_when_unfiltered() {
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
        let filters = Filters::from_config(&config);
        let selected = select_sources(&config, &filters).unwrap();
        let folders: Vec<_> = selected.iter().map(|s| s.folder.as_str()).collect();
        assert_eq!(folders, vec!["US_CA", "US_OR"]);
    }

    #[test]
    fn classify_folder_recognizes_opencpn_layout() {
        assert_eq!(classify_folder("US"), Some(SourceKind::All));
        assert_eq!(
            classify_folder("US_CGD13"),
            Some(SourceKind::CoastGuardDistrict("13".into()))
        );
        assert_eq!(
            classify_folder("US_REGION14"),
            Some(SourceKind::Region("14".into()))
        );
        assert_eq!(
            classify_folder("US_CA"),
            Some(SourceKind::State("CA".into()))
        );
        assert_eq!(classify_folder("US_INLAND_BUOYS"), Some(SourceKind::InlandBuoys));
    }
}
