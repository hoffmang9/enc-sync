//! OpenCPN ENC folder taxonomy shared by runtime selection and build-time validation.

use quick_xml::events::Event;
use quick_xml::Reader;

pub const OPENCPN_CATALOG_DIR_PREFIX: &str = "{USERDATA}/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenCPNChartSourceRecord {
    pub name: String,
    pub catalog_url: String,
    pub catalog_filename: String,
    pub folder: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EncFolderClass {
    All,
    Inland,
    State,
    Region,
    CoastGuardDistrict,
}

/// Extract the ENC folder name from an OpenCPN catalog `<dir>` value.
pub fn folder_from_opencpn_dir(dir: &str) -> Option<&str> {
    let rel = dir
        .strip_prefix(OPENCPN_CATALOG_DIR_PREFIX)?
        .trim_start_matches('/');
    rel.rsplit('/').next()
}

pub(crate) fn classify_enc_folder(folder: &str) -> Option<(EncFolderClass, &str)> {
    if folder == "US" {
        return Some((EncFolderClass::All, ""));
    }
    if matches!(
        folder,
        "US_INLAND" | "US_INLAND_BUOYS" | "US_INLAND_OVERLAYS"
    ) {
        return Some((EncFolderClass::Inland, ""));
    }
    if let Some(code) = folder.strip_prefix("US_CGD") {
        return Some((EncFolderClass::CoastGuardDistrict, code));
    }
    if let Some(code) = folder.strip_prefix("US_REGION") {
        return Some((EncFolderClass::Region, code));
    }
    if let Some(code) = folder.strip_prefix("US_") {
        return Some((EncFolderClass::State, code));
    }
    None
}

pub fn recognized_enc_folder(folder: &str) -> bool {
    classify_enc_folder(folder).is_some()
}

/// Parse recognized chart sources from OpenCPN's embedded `chart_sources.xml` format.
#[allow(dead_code)] // used by build.rs via `#[path]` include; tests call through the library
pub fn parse_opencpn_chart_sources(xml: &str) -> Result<Vec<OpenCPNChartSourceRecord>, String> {
    let mut reader = Reader::from_str(xml);
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
                current_text.push_str(&text.unescape().map_err(|e| e.to_string())?);
            }
            Ok(Event::End(tag)) => {
                let field = String::from_utf8_lossy(tag.name().as_ref()).into_owned();
                if field == "catalog" {
                    if let (Some(name), Some(location), Some(dir)) = (
                        current_name.take(),
                        current_location.take(),
                        current_dir.take(),
                    ) {
                        if let Some(record) = chart_source_record(name, location, dir) {
                            sources.push(record);
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
            Err(error) => return Err(error.to_string()),
        }
        buf.clear();
    }

    sources.sort_by(|a, b| a.folder.cmp(&b.folder));
    Ok(sources)
}

fn chart_source_record(
    name: String,
    catalog_url: String,
    dir: String,
) -> Option<OpenCPNChartSourceRecord> {
    let folder = folder_from_opencpn_dir(&dir)?.to_string();
    if !recognized_enc_folder(&folder) {
        return None;
    }
    Some(OpenCPNChartSourceRecord {
        catalog_filename: catalog_filename(&catalog_url).to_string(),
        name,
        catalog_url,
        folder,
    })
}

fn catalog_filename(url: &str) -> &str {
    url.rsplit('/')
        .next()
        .filter(|part| !part.is_empty())
        .unwrap_or("catalog.xml")
}

#[cfg(test)]
mod tests {
    use super::{folder_from_opencpn_dir, parse_opencpn_chart_sources, recognized_enc_folder};

    const OPENCPN_SOURCES_XML: &str = include_str!("../data/opencpn_enc_sources.xml");

    #[test]
    fn folder_from_opencpn_dir_extracts_enc_folder_name() {
        assert_eq!(
            folder_from_opencpn_dir("{USERDATA}/ENC/US_CA"),
            Some("US_CA")
        );
    }

    #[test]
    fn recognized_enc_folder_recognizes_opencpn_layout() {
        assert!(recognized_enc_folder("US"));
        assert!(recognized_enc_folder("US_CGD13"));
        assert!(recognized_enc_folder("US_REGION14"));
        assert!(recognized_enc_folder("US_CA"));
        assert!(recognized_enc_folder("US_INLAND_BUOYS"));
        assert!(!recognized_enc_folder("ignored"));
    }

    #[test]
    fn parse_opencpn_chart_sources_reads_embedded_catalogs() {
        let sources = parse_opencpn_chart_sources(OPENCPN_SOURCES_XML).unwrap();
        assert!(!sources.is_empty());
        assert!(sources.iter().any(|source| source.folder == "US_CA"));
        assert!(sources.iter().any(|source| source.folder == "US"));
        assert_eq!(
            sources
                .iter()
                .find(|source| source.folder == "US_CA")
                .map(|source| source.catalog_filename.as_str()),
            Some("CA_ENCProdCat.xml")
        );
    }

    #[test]
    fn parse_opencpn_chart_sources_returns_sorted_folders() {
        let sources = parse_opencpn_chart_sources(OPENCPN_SOURCES_XML).unwrap();
        let folders: Vec<_> = sources
            .iter()
            .map(|source| source.folder.as_str())
            .collect();
        let mut sorted = folders.clone();
        sorted.sort_unstable();
        assert_eq!(folders, sorted);
    }
}
