//! OpenCPN ENC folder taxonomy used by build-time source codegen.

use super::source_norm::normalize_numeric_code;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderKind {
    All,
    State(String),
    Region(String),
    CoastGuardDistrict(String),
    InlandMain,
    InlandBuoys,
    InlandOverlays,
}

pub fn classify_folder(folder: &str) -> Option<FolderKind> {
    if folder == "US" {
        return Some(FolderKind::All);
    }
    if folder == "US_INLAND" {
        return Some(FolderKind::InlandMain);
    }
    if folder == "US_INLAND_BUOYS" {
        return Some(FolderKind::InlandBuoys);
    }
    if folder == "US_INLAND_OVERLAYS" {
        return Some(FolderKind::InlandOverlays);
    }
    if let Some(code) = folder.strip_prefix("US_CGD") {
        return Some(FolderKind::CoastGuardDistrict(normalize_numeric_code(code)));
    }
    if let Some(code) = folder.strip_prefix("US_REGION") {
        return Some(FolderKind::Region(normalize_numeric_code(code)));
    }
    if let Some(code) = folder.strip_prefix("US_") {
        return Some(FolderKind::State(code.to_ascii_uppercase()));
    }
    None
}

pub fn folder_kind_rust_expr(kind: &FolderKind) -> String {
    match kind {
        FolderKind::All => "SourceKind::All".to_string(),
        FolderKind::InlandMain => "SourceKind::InlandMain".to_string(),
        FolderKind::InlandBuoys => "SourceKind::InlandBuoys".to_string(),
        FolderKind::InlandOverlays => "SourceKind::InlandOverlays".to_string(),
        FolderKind::State(code) => format!("SourceKind::State({code:?})"),
        FolderKind::Region(code) => format!("SourceKind::Region({code:?})"),
        FolderKind::CoastGuardDistrict(code) => {
            format!("SourceKind::CoastGuardDistrict({code:?})")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_folder, FolderKind};

    #[test]
    fn classify_folder_recognizes_opencpn_layout() {
        assert_eq!(classify_folder("US"), Some(FolderKind::All));
        assert_eq!(
            classify_folder("US_CGD13"),
            Some(FolderKind::CoastGuardDistrict("13".into()))
        );
        assert_eq!(
            classify_folder("US_REGION14"),
            Some(FolderKind::Region("14".into()))
        );
        assert_eq!(
            classify_folder("US_CA"),
            Some(FolderKind::State("CA".into()))
        );
        assert_eq!(
            classify_folder("US_INLAND_BUOYS"),
            Some(FolderKind::InlandBuoys)
        );
    }
}
