//! OpenCPN ENC folder taxonomy shared by runtime selection and build-time validation.

use super::source_norm::normalize_numeric_code;

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

pub fn source_kind_from_folder(folder: &str) -> Option<SourceKind> {
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
        let code = normalize_numeric_code(code);
        return Some(SourceKind::CoastGuardDistrict(code));
    }
    if let Some(code) = folder.strip_prefix("US_REGION") {
        let code = normalize_numeric_code(code);
        return Some(SourceKind::Region(code));
    }
    if let Some(code) = folder.strip_prefix("US_") {
        let code = code.to_ascii_uppercase();
        return Some(SourceKind::State(code));
    }
    None
}

/// Used by `build.rs` when parsing embedded OpenCPN source XML.
#[allow(dead_code)]
pub fn recognized_enc_folder(folder: &str) -> bool {
    source_kind_from_folder(folder).is_some()
}

#[cfg(test)]
mod tests {
    use super::{source_kind_from_folder, SourceKind};

    #[test]
    fn source_kind_from_folder_recognizes_opencpn_layout() {
        assert_eq!(source_kind_from_folder("US"), Some(SourceKind::All));
        assert_eq!(
            source_kind_from_folder("US_CGD13"),
            Some(SourceKind::CoastGuardDistrict("13".into()))
        );
        assert_eq!(
            source_kind_from_folder("US_REGION14"),
            Some(SourceKind::Region("14".into()))
        );
        assert_eq!(
            source_kind_from_folder("US_CA"),
            Some(SourceKind::State("CA".into()))
        );
        assert_eq!(
            source_kind_from_folder("US_INLAND_BUOYS"),
            Some(SourceKind::InlandBuoys)
        );
    }

    #[test]
    fn source_kind_from_folder_normalizes_numeric_codes() {
        assert_eq!(
            source_kind_from_folder("US_CGD01"),
            Some(SourceKind::CoastGuardDistrict("1".into()))
        );
    }
}
