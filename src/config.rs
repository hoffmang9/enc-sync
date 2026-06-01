use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub chart_dir: PathBuf,
    #[serde(default = "default_catalog_url")]
    pub catalog_url: String,
    /// Two-letter state codes to keep current (e.g. "CA", "WA"). Empty = no state filter.
    #[serde(default)]
    pub states: Vec<String>,
    /// NOAA region numbers as strings (e.g. "14", "15"). Empty = no region filter.
    #[serde(default)]
    pub regions: Vec<String>,
    /// Coast Guard district numbers as strings (e.g. "11", "13"). Empty = no CGD filter.
    #[serde(default)]
    pub coast_guard_districts: Vec<String>,
    #[serde(default = "default_true")]
    pub restart_opencpn: bool,
    #[serde(default = "default_true")]
    pub rebuild_chart_db: bool,
}

pub fn default_catalog_url() -> String {
    "https://www.charts.noaa.gov/ENCs/ENCProdCat.xml".to_string()
}

fn default_true() -> bool {
    true
}

pub fn load_config(path: &Path) -> Result<Config> {
    let text =
        fs::read_to_string(path).with_context(|| format!("reading config {}", path.display()))?;
    let mut config: Config =
        toml::from_str(&text).with_context(|| format!("parsing config {}", path.display()))?;
    config.chart_dir = validate_chart_dir(&config.chart_dir)?;
    Ok(config)
}

fn validate_chart_dir(path: &Path) -> Result<PathBuf> {
    let expanded = expand_path(path)?;
    if chart_dir_is_blank(&expanded) {
        bail!("chart_dir must not be empty");
    }
    if expanded.is_file() {
        bail!(
            "chart_dir {} is a file, not a directory",
            expanded.display()
        );
    }
    Ok(expanded)
}

pub(crate) fn prepare_chart_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("creating chart directory {}", path.display()))
}

fn chart_dir_is_blank(path: &Path) -> bool {
    path.as_os_str().is_empty() || path.to_str().is_some_and(|s| s.trim().is_empty())
}

pub fn expand_path(path: &Path) -> Result<PathBuf> {
    let Some(raw) = path.to_str() else {
        return Ok(path.to_path_buf());
    };
    if !raw.starts_with('~') {
        return Ok(path.to_path_buf());
    }
    if home_dir().is_none() {
        anyhow::bail!("cannot expand '{raw}': home directory is not set");
    }
    Ok(PathBuf::from(shellexpand::tilde(raw).into_owned()))
}

/// User home directory, matching [`shellexpand::tilde`] (`USERPROFILE` on Windows, `HOME` elsewhere).
pub fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn toml_path(path: &Path) -> String {
        path.display().to_string().replace('\\', "/")
    }

    #[test]
    fn load_config_expands_tilde_in_chart_dir() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            r#"
chart_dir = "~/Charts/ENC/US"
"#,
        )
        .unwrap();

        #[cfg(not(windows))]
        {
            let _home = crate::test_env::EnvGuard::override_home(dir.path());
            let config = load_config(&path).unwrap();
            assert_eq!(config.chart_dir, dir.path().join("Charts/ENC/US"));
        }

        #[cfg(windows)]
        {
            let config = load_config(&path).unwrap();
            let expected = home_dir()
                .expect("Windows profile directory")
                .join("Charts/ENC/US");
            assert_eq!(config.chart_dir, expected);
        }
    }

    #[test]
    fn load_config_parses_example_fields() {
        let dir = TempDir::new().unwrap();
        let chart_dir = dir.path().join("charts/enc");
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            format!(
                r#"
chart_dir = "{}"
states = ["CA", "OR"]
regions = ["14"]
coast_guard_districts = ["11"]
restart_opencpn = false
"#,
                toml_path(&chart_dir)
            ),
        )
        .unwrap();

        let config = load_config(&path).unwrap();
        assert_eq!(config.chart_dir, chart_dir);
        assert_eq!(config.states, vec!["CA", "OR"]);
        assert_eq!(config.regions, vec!["14"]);
        assert_eq!(config.coast_guard_districts, vec!["11"]);
        assert!(!config.restart_opencpn);
        assert!(config.rebuild_chart_db);
        assert!(config.catalog_url.contains("ENCProdCat.xml"));
    }

    #[test]
    fn load_config_rejects_empty_chart_dir() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, r#"chart_dir = """#).unwrap();

        let error = load_config(&path).unwrap_err();
        assert!(
            error.to_string().contains("chart_dir must not be empty"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn load_config_rejects_whitespace_chart_dir() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, r#"chart_dir = "   ""#).unwrap();

        let error = load_config(&path).unwrap_err();
        assert!(
            error.to_string().contains("chart_dir must not be empty"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn load_config_accepts_missing_chart_dir() {
        let dir = TempDir::new().unwrap();
        let chart_dir = dir.path().join("Charts/ENC/US");
        assert!(!chart_dir.exists());

        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            format!(
                r#"
chart_dir = "{}"
"#,
                toml_path(&chart_dir)
            ),
        )
        .unwrap();

        let config = load_config(&path).unwrap();
        assert_eq!(config.chart_dir, chart_dir);
        assert!(!chart_dir.exists());
    }

    #[test]
    fn prepare_chart_dir_creates_missing_path() {
        let dir = TempDir::new().unwrap();
        let chart_dir = dir.path().join("Charts/ENC/US");
        assert!(!chart_dir.exists());

        prepare_chart_dir(&chart_dir).unwrap();
        assert!(chart_dir.is_dir());
    }

    #[test]
    fn load_config_rejects_chart_dir_that_is_a_file() {
        let dir = TempDir::new().unwrap();
        let chart_path = dir.path().join("not-a-dir");
        fs::write(&chart_path, b"file").unwrap();

        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            format!(
                r#"
chart_dir = "{}"
"#,
                toml_path(&chart_path)
            ),
        )
        .unwrap();

        let error = load_config(&path).unwrap_err();
        assert!(
            error.to_string().contains("is a file, not a directory"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn expand_path_errors_when_home_missing() {
        let _home = crate::test_env::EnvGuard::clear_home_dirs();
        if home_dir().is_some() {
            return;
        }
        assert!(expand_path(Path::new("~/Charts")).is_err());
    }

    #[test]
    fn expand_path_leaves_non_leading_tilde_literal() {
        let _home = crate::test_env::EnvGuard::clear_home_dirs();
        let path = Path::new("/tmp/chart~archive");

        assert_eq!(expand_path(path).unwrap(), path);
    }
}
