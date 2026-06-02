//! OpenCPN ENC folder taxonomy shared by runtime selection and build-time validation.

pub const OPENCPN_CATALOG_DIR_PREFIX: &str = "{USERDATA}/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EncFolderClass {
    All,
    Inland,
    State,
    Region,
    CoastGuardDistrict,
}

/// Extract the ENC folder name from an OpenCPN catalog `<dir>` value.
#[allow(dead_code)] // used by build.rs via `#[path]` include
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

/// Used by `build.rs` when parsing embedded OpenCPN source XML.
#[allow(dead_code)] // only referenced from build.rs via `#[path]` include
pub fn recognized_enc_folder(folder: &str) -> bool {
    classify_enc_folder(folder).is_some()
}

#[cfg(test)]
pub(crate) fn count_recognized_catalogs_in_xml(xml: &str) -> Result<usize, String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut in_catalog = false;
    let mut current_field = None::<String>;
    let mut current_text = String::new();
    let mut current_name = None::<String>;
    let mut current_location = None::<String>;
    let mut current_dir = None::<String>;
    let mut count = 0usize;

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
                    if let (Some(_name), Some(_location), Some(dir)) = (
                        current_name.take(),
                        current_location.take(),
                        current_dir.take(),
                    ) {
                        if let Some(folder) = folder_from_opencpn_dir(&dir) {
                            if recognized_enc_folder(folder) {
                                count += 1;
                            }
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

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::{folder_from_opencpn_dir, recognized_enc_folder};

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
}
