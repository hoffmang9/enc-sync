//! Numeric code normalization shared by runtime selection and build-time codegen.

pub fn normalize_numeric_code(raw: &str) -> String {
    let trimmed = raw.trim().to_ascii_uppercase();
    let stripped = trimmed.trim_start_matches('0');
    if stripped.is_empty() {
        "0".to_string()
    } else {
        stripped.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_numeric_code;

    #[test]
    fn normalize_numeric_code_trims_leading_zeros() {
        assert_eq!(normalize_numeric_code("01"), "1");
        assert_eq!(normalize_numeric_code("00"), "0");
        assert_eq!(normalize_numeric_code(" 14 "), "14");
    }
}
