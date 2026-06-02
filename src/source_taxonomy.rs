//! OpenCPN ENC folder taxonomy shared by runtime selection and build-time validation.
//!
//! This file is compiled into both the library and `build.rs`; each consumer uses a
//! different subset of helpers, so dead-code warnings are suppressed here.
#![allow(dead_code)]

use std::collections::HashSet;

use super::source_norm::normalize_numeric_code;

/// Config minimums used when deciding whether a folder is selected.
pub struct SelectionCriteria<'a> {
    pub states: &'a HashSet<String>,
    pub regions: &'a HashSet<String>,
    pub coast_guard_districts: &'a HashSet<String>,
    pub all_enc: bool,
    pub inland: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EncFolderClass {
    All,
    Inland,
    State,
    Region,
    CoastGuardDistrict,
}

fn classify_enc_folder(folder: &str) -> Option<(EncFolderClass, &str)> {
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

pub fn folder_matches_selection(folder: &str, criteria: &SelectionCriteria<'_>) -> bool {
    let Some((class, code)) = classify_enc_folder(folder) else {
        return false;
    };
    match class {
        EncFolderClass::All => criteria.all_enc,
        EncFolderClass::Inland => criteria.inland,
        EncFolderClass::State => {
            let code = code.trim().to_ascii_uppercase();
            criteria.states.contains(&code)
        }
        EncFolderClass::Region => criteria.regions.contains(&normalize_numeric_code(code)),
        EncFolderClass::CoastGuardDistrict => criteria
            .coast_guard_districts
            .contains(&normalize_numeric_code(code)),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{folder_matches_selection, recognized_enc_folder, SelectionCriteria};

    fn criteria<'a>(
        states: &'a HashSet<String>,
        regions: &'a HashSet<String>,
        coast_guard_districts: &'a HashSet<String>,
        all_enc: bool,
        inland: bool,
    ) -> SelectionCriteria<'a> {
        SelectionCriteria {
            states,
            regions,
            coast_guard_districts,
            all_enc,
            inland,
        }
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
    fn folder_matches_selection_respects_config_minimums() {
        let states = HashSet::from(["CA".to_string()]);
        let regions = HashSet::from(["14".to_string()]);
        let districts = HashSet::from(["1".to_string()]);
        let selection = criteria(&states, &regions, &districts, false, false);

        assert!(folder_matches_selection("US_CA", &selection));
        assert!(!folder_matches_selection("US_OR", &selection));
        assert!(folder_matches_selection("US_REGION14", &selection));
        assert!(folder_matches_selection("US_CGD01", &selection));
        assert!(!folder_matches_selection("US", &selection));
        assert!(!folder_matches_selection("US_INLAND", &selection));
    }
}
