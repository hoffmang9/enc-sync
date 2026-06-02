use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    /// OpenCPN base chart directory (`BaseChartDir`). ENC sources live under
    /// `{chart_dir}/ENC/…` using OpenCPN's default folder names.
    pub chart_dir: PathBuf,
    /// Two-letter state codes → sync matching `ENC/US_XX` folders.
    #[serde(default)]
    pub states: Vec<String>,
    /// NOAA region numbers as strings → sync matching `ENC/US_REGIONxx` folders.
    #[serde(default)]
    pub regions: Vec<String>,
    /// Coast Guard district numbers as strings → sync matching `ENC/US_CGDxx` folders.
    #[serde(default)]
    pub coast_guard_districts: Vec<String>,
    /// Sync the national `ENC/US` folder (`ENCProdCat.xml`).
    #[serde(default)]
    pub all_enc: bool,
    /// Sync US Army Corps inland ENC folders (`US_INLAND`, `US_INLAND_BUOYS`,
    /// `US_INLAND_OVERLAYS`).
    #[serde(default)]
    pub inland: bool,
    /// Optional base URL for catalog downloads (testing or mirrors). When set,
    /// each source loads `{catalog_base_url}/{catalog_filename}` instead of its
    /// default NOAA/ACE URL.
    #[serde(default)]
    pub catalog_base_url: Option<String>,
    #[serde(default = "default_true")]
    pub restart_opencpn: bool,
    #[serde(default = "default_true")]
    pub rebuild_chart_db: bool,
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
chart_dir = "~/Documents/Charts"
"#,
        )
        .unwrap();

        #[cfg(not(windows))]
        {
            let _home = crate::test_env::EnvGuard::override_home(dir.path());
            let config = load_config(&path).unwrap();
            assert_eq!(config.chart_dir, dir.path().join("Documents/Charts"));
        }

        #[cfg(windows)]
        {
            let config = load_config(&path).unwrap();
            let expected = home_dir()
                .expect("Windows profile directory")
                .join("Documents/Charts");
            assert_eq!(config.chart_dir, expected);
        }
    }

    #[test]
    fn load_config_parses_example_fields() {
        let dir = TempDir::new().unwrap();
        let chart_dir = dir.path().join("Documents/Charts");
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            format!(
                r#"
chart_dir = "{}"
states = ["CA", "OR"]
regions = ["14"]
coast_guard_districts = ["11"]
all_enc = true
inland = true
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
        assert!(config.all_enc);
        assert!(config.inland);
        assert!(!config.restart_opencpn);
        assert!(config.rebuild_chart_db);
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
    fn prepare_chart_dir_creates_missing_path() {
        let dir = TempDir::new().unwrap();
        let chart_dir = dir.path().join("Documents/Charts/ENC/US_CA");
        assert!(!chart_dir.exists());

        prepare_chart_dir(&chart_dir).unwrap();
        assert!(chart_dir.is_dir());
    }
}
