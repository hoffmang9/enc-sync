//! OpenCPN ENC folder taxonomy shared by runtime selection and build-time validation.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EncFolderClass {
    All,
    Inland,
    State,
    Region,
    CoastGuardDistrict,
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
mod tests {
    use super::recognized_enc_folder;

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
