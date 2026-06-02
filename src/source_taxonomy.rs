//! Build-time folder taxonomy helpers shared with `build.rs`.

use super::source_norm::normalize_numeric_code;

pub fn source_kind_rust_expr_for_folder(folder: &str) -> Option<String> {
    if folder == "US" {
        return Some("SourceKind::All".to_string());
    }
    if folder == "US_INLAND" {
        return Some("SourceKind::InlandMain".to_string());
    }
    if folder == "US_INLAND_BUOYS" {
        return Some("SourceKind::InlandBuoys".to_string());
    }
    if folder == "US_INLAND_OVERLAYS" {
        return Some("SourceKind::InlandOverlays".to_string());
    }
    if let Some(code) = folder.strip_prefix("US_CGD") {
        let code = normalize_numeric_code(code);
        return Some(format!("SourceKind::CoastGuardDistrict({code:?})"));
    }
    if let Some(code) = folder.strip_prefix("US_REGION") {
        let code = normalize_numeric_code(code);
        return Some(format!("SourceKind::Region({code:?})"));
    }
    if let Some(code) = folder.strip_prefix("US_") {
        let code = code.to_ascii_uppercase();
        return Some(format!("SourceKind::State({code:?})"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::source_kind_rust_expr_for_folder;

    #[test]
    fn source_kind_expr_recognizes_opencpn_layout() {
        assert_eq!(
            source_kind_rust_expr_for_folder("US"),
            Some("SourceKind::All".to_string())
        );
        assert_eq!(
            source_kind_rust_expr_for_folder("US_CGD13"),
            Some("SourceKind::CoastGuardDistrict(\"13\")".to_string())
        );
        assert_eq!(
            source_kind_rust_expr_for_folder("US_REGION14"),
            Some("SourceKind::Region(\"14\")".to_string())
        );
        assert_eq!(
            source_kind_rust_expr_for_folder("US_CA"),
            Some("SourceKind::State(\"CA\")".to_string())
        );
        assert_eq!(
            source_kind_rust_expr_for_folder("US_INLAND_BUOYS"),
            Some("SourceKind::InlandBuoys".to_string())
        );
    }

    #[test]
    fn source_kind_expr_normalizes_numeric_codes() {
        assert_eq!(
            source_kind_rust_expr_for_folder("US_CGD01"),
            Some("SourceKind::CoastGuardDistrict(\"1\")".to_string())
        );
    }
}
