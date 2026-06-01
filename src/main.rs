//! Sync NOAA ENC charts for OpenCPN.
//!
//! NOAA publishes ENC updates every weekday evening; run after that (most days
//! the binary exits quickly when nothing changed). Example cron:
//!   0 23 * * 1-5 enc-sync --config ~/.config/enc-sync/config.toml \
//!     >>/tmp/enc-sync.log 2>&1

use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{copy, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use chrono::DateTime;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Deserialize;
use zip::read::ZipArchive;

const CATALOG_FILENAME: &str = "ENCProdCat.xml";
const UPDATE_DATA_FILENAME: &str = "chartdldr_pi.dat";
const USER_AGENT: &str = "enc-sync/0.1";

#[derive(Debug, Deserialize)]
struct Config {
    chart_dir: PathBuf,
    #[serde(default = "default_catalog_url")]
    catalog_url: String,
    /// Two-letter state codes to keep current (e.g. "CA", "WA"). Empty = no state filter.
    #[serde(default)]
    states: Vec<String>,
    /// NOAA region numbers as strings (e.g. "14", "15"). Empty = no region filter.
    #[serde(default)]
    regions: Vec<String>,
    /// Coast Guard district numbers as strings (e.g. "11", "13"). Empty = no CGD filter.
    #[serde(default)]
    coast_guard_districts: Vec<String>,
    #[serde(default = "default_true")]
    restart_opencpn: bool,
    #[serde(default = "default_true")]
    rebuild_chart_db: bool,
}

#[derive(Debug, Clone)]
struct Filters {
    states: HashSet<String>,
    regions: HashSet<String>,
    coast_guard_districts: HashSet<String>,
}

impl Filters {
    fn from_config(config: &Config) -> Self {
        Self {
            states: normalize_codes(&config.states),
            regions: normalize_codes(&config.regions),
            coast_guard_districts: normalize_codes(&config.coast_guard_districts),
        }
    }

    fn is_active(&self) -> bool {
        !self.states.is_empty() || !self.regions.is_empty() || !self.coast_guard_districts.is_empty()
    }

    fn matches(&self, cell: &Cell) -> bool {
        if !self.is_active() {
            return true;
        }
        if !self.states.is_empty()
            && cell
                .states
                .iter()
                .any(|state| self.states.contains(&state.to_ascii_uppercase()))
        {
            return true;
        }
        if !self.regions.is_empty()
            && cell
                .regions
                .iter()
                .any(|region| self.regions.contains(region))
        {
            return true;
        }
        if !self.coast_guard_districts.is_empty()
            && cell.coast_guard_districts.iter().any(|cgd| self.coast_guard_districts.contains(cgd))
        {
            return true;
        }
        false
    }
}

#[derive(Debug, Clone)]
struct Cell {
    name: String,
    url: String,
    timestamp: i64,
    states: Vec<String>,
    regions: Vec<String>,
    coast_guard_districts: Vec<String>,
}

fn default_catalog_url() -> String {
    "https://www.charts.noaa.gov/ENCs/ENCProdCat.xml".to_string()
}

fn default_true() -> bool {
    true
}

fn normalize_codes(values: &[String]) -> HashSet<String> {
    values
        .iter()
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| !value.is_empty())
        .collect()
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config_path = parse_config_path()?;
    let config = load_config(&config_path)?;
    let filters = Filters::from_config(&config);

    if filters.is_active() {
        log::info!(
            "Filters active: {} state(s), {} region(s), {} CG district(s)",
            filters.states.len(),
            filters.regions.len(),
            filters.coast_guard_districts.len()
        );
    } else {
        log::info!("No area filters configured; all catalog cells are eligible");
    }

    let catalog_path = download_catalog(&config)?;
    let cells = parse_catalog(&catalog_path)?;
    log::info!("Catalog lists {} ENC cells", cells.len());

    let mut update_data = load_update_data(&config.chart_dir)?;
    let pending: Vec<Cell> = cells
        .into_iter()
        .filter(|cell| filters.matches(cell))
        .filter(|cell| needs_update(&config.chart_dir, cell, &update_data))
        .collect();

    let mut failed = Vec::new();
    let mut updated = 0usize;

    if pending.is_empty() {
        log::info!("All filtered catalog cells are up to date on disk");
    } else {
        log::info!("Downloading {} updated or new cells", pending.len());
        for cell in pending {
            match download_cell(&config.chart_dir, &cell) {
                Ok(()) => {
                    update_data.insert(cell.name.to_ascii_lowercase(), cell.timestamp);
                    save_update_data(&config.chart_dir, &update_data)?;
                    updated += 1;
                }
                Err(error) => {
                    log::error!("Failed to update {}: {:#}", cell.name, error);
                    failed.push(cell.name);
                }
            }
        }
        if !failed.is_empty() {
            let preview: Vec<_> = failed.iter().take(10).cloned().collect();
            log::error!("{} cells failed: {}", failed.len(), preview.join(", "));
            if failed.len() > 10 {
                log::error!("... and {} more", failed.len() - 10);
            }
        }
    }

    if updated > 0 && config.restart_opencpn {
        restart_opencpn(config.rebuild_chart_db)?;
    } else if updated == 0 {
        log::info!("No chart files changed; leaving OpenCPN running");
    }

    if failed.is_empty() {
        Ok(())
    } else {
        bail!("{} cell(s) failed to update", failed.len())
    }
}

fn parse_config_path() -> Result<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    for (index, arg) in args.iter().enumerate() {
        if arg == "--config" {
            return args
                .get(index + 1)
                .map(PathBuf::from)
                .context("--config requires a path argument");
        }
        if arg == "-h" || arg == "--help" {
            print_help();
            std::process::exit(0);
        }
    }
    if let Some(home) = home_config_path() {
        if home.is_file() {
            return Ok(home);
        }
    }
    for candidate in [PathBuf::from("enc-sync.toml")] {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    bail!(
        "No config file found. Pass --config /path/to/enc-sync.toml\n\n{}",
        help_text()
    )
}

fn home_config_path() -> Option<PathBuf> {
    Some(dirs_home()?.join(".config/enc-sync/config.toml"))
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn print_help() {
    print!("{}", help_text());
}

fn help_text() -> &'static str {
    r#"enc-sync -- Sync NOAA ENC charts for OpenCPN

Usage:
  enc-sync --config /path/to/enc-sync.toml

If --config is omitted, looks for enc-sync.toml in the current directory,
then ~/.config/enc-sync/config.toml.

See enc-sync.example.toml for configuration options.
"#
}

fn load_config(path: &Path) -> Result<Config> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("reading config {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing config {}", path.display()))
}

fn parse_catalog_timestamp(raw: &str) -> Result<i64> {
    let raw = raw.trim();
    if let Ok(parsed) = DateTime::parse_from_rfc3339(raw) {
        return Ok(parsed.timestamp());
    }
    if raw.ends_with('Z') {
        let parsed = DateTime::parse_from_rfc3339(&format!("{}+00:00", &raw[..raw.len() - 1]))
            .with_context(|| format!("parsing catalog timestamp '{raw}'"))?;
        return Ok(parsed.timestamp());
    }
    bail!("parsing catalog timestamp '{raw}'")
}

fn load_update_data(chart_dir: &Path) -> Result<BTreeMap<String, i64>> {
    let path = chart_dir.join(UPDATE_DATA_FILENAME);
    let mut data = BTreeMap::new();
    if !path.is_file() {
        return Ok(data);
    }
    for line in fs::read_to_string(&path)?.lines() {
        let mut parts = line.split_whitespace();
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            if let Ok(timestamp) = value.parse::<i64>() {
                data.insert(key.to_ascii_lowercase(), timestamp);
            }
        }
    }
    Ok(data)
}

fn save_update_data(chart_dir: &Path, data: &BTreeMap<String, i64>) -> Result<()> {
    let path = chart_dir.join(UPDATE_DATA_FILENAME);
    let mut file = File::create(&path)?;
    for (key, value) in data {
        writeln!(file, "{key} {value}")?;
    }
    Ok(())
}

fn local_cell_mtime(chart_dir: &Path, cell_name: &str) -> Option<i64> {
    fn mtime(path: &Path) -> Option<i64> {
        path.metadata()
            .ok()?
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_secs() as i64)
    }

    let direct = chart_dir.join(cell_name);
    if direct.is_dir() {
        return mtime(&direct);
    }

    let lower = cell_name.to_ascii_lowercase();
    fs::read_dir(chart_dir).ok()?.filter_map(Result::ok).find_map(|entry| {
        if entry.file_type().ok()?.is_dir()
            && entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(&lower)
        {
            mtime(&entry.path())
        } else {
            None
        }
    })
}

fn exists_locally(chart_dir: &Path, cell_name: &str, update_data: &BTreeMap<String, i64>) -> bool {
    let key = cell_name.to_ascii_lowercase();
    if update_data.contains_key(&key) {
        return true;
    }
    local_cell_mtime(chart_dir, cell_name).is_some()
}

fn needs_update(chart_dir: &Path, cell: &Cell, update_data: &BTreeMap<String, i64>) -> bool {
    if !exists_locally(chart_dir, &cell.name, update_data) {
        return true;
    }
    let key = cell.name.to_ascii_lowercase();
    let local_ts = update_data
        .get(&key)
        .copied()
        .or_else(|| local_cell_mtime(chart_dir, &cell.name));
    match local_ts {
        None => true,
        Some(local_ts) => cell.timestamp > local_ts,
    }
}

fn fetch_url(url: &str) -> Result<Vec<u8>> {
    let mut response = ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .with_context(|| format!("GET {url}"))?;
    response
        .body_mut()
        .read_to_vec()
        .with_context(|| format!("reading response body from {url}"))
}

fn download_catalog(config: &Config) -> Result<PathBuf> {
    fs::create_dir_all(&config.chart_dir)?;
    let catalog_path = config.chart_dir.join(CATALOG_FILENAME);
    let tmp_path = config
        .chart_dir
        .join(format!(".catalog-{}.xml", std::process::id()));

    log::info!("Downloading catalog {}", config.catalog_url);
    let result = (|| -> Result<()> {
        let bytes = fetch_url(&config.catalog_url)?;
        fs::write(&tmp_path, bytes)?;
        fs::rename(&tmp_path, &catalog_path)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    result?;
    log::info!("Catalog saved to {}", catalog_path.display());
    Ok(catalog_path)
}

fn parse_catalog(catalog_path: &Path) -> Result<Vec<Cell>> {
    let xml = fs::read_to_string(catalog_path)?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);

    let mut cells = Vec::new();
    let mut buf = Vec::new();
    let mut in_cell = false;
    let mut current: Option<CellBuilder> = None;
    let mut nested = None::<NestedField>;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(tag)) => {
                let name = tag.name().as_ref().to_vec();
                let tag_name = String::from_utf8_lossy(&name).into_owned();
                match tag_name.as_str() {
                    "cell" => {
                        in_cell = true;
                        current = Some(CellBuilder::default());
                    }
                    "state" if in_cell => nested = Some(NestedField::State),
                    "region" if in_cell => nested = Some(NestedField::Region),
                    "coast_guard_district" if in_cell => nested = Some(NestedField::Cgd),
                    field if in_cell => {
                        if let Some(builder) = current.as_mut() {
                            builder.start_field(field);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(text)) => {
                let value = text.unescape()?.into_owned();
                if let Some(field) = nested.take() {
                    if let Some(builder) = current.as_mut() {
                        builder.push_nested(field, value);
                    }
                    continue;
                }
                if in_cell {
                    if let Some(builder) = current.as_mut() {
                        builder.push_text(value);
                    }
                }
            }
            Ok(Event::End(tag)) => {
                let name = tag.name().as_ref().to_vec();
                let tag_name = String::from_utf8_lossy(&name).into_owned();
                match tag_name.as_str() {
                    "state" | "region" | "coast_guard_district" => {
                        nested = None;
                    }
                    "cell" => {
                        if let Some(builder) = current.take() {
                            if let Some(cell) = builder.finish()? {
                                cells.push(cell);
                            }
                        }
                        in_cell = false;
                    }
                    field if in_cell => {
                        if let Some(builder) = current.as_mut() {
                            builder.end_field(field);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buf.clear();
    }

    Ok(cells)
}

#[derive(Clone, Copy)]
enum NestedField {
    State,
    Region,
    Cgd,
}

#[derive(Default)]
struct CellBuilder {
    name: Option<String>,
    url: Option<String>,
    timestamp: Option<i64>,
    states: Vec<String>,
    regions: Vec<String>,
    coast_guard_districts: Vec<String>,
    current_field: Option<String>,
    current_text: String,
}

impl CellBuilder {
    fn start_field(&mut self, field: &str) {
        self.current_field = Some(field.to_string());
        self.current_text.clear();
    }

    fn push_text(&mut self, value: String) {
        self.current_text.push_str(&value);
    }

    fn end_field(&mut self, field: &str) {
        let Some(active) = self.current_field.as_deref() else {
            return;
        };
        if active != field {
            return;
        }
        let value = std::mem::take(&mut self.current_text);
        self.current_field = None;
        if value.trim().is_empty() {
            return;
        }
        match field {
            "name" => self.name = Some(value.trim().to_string()),
            "zipfile_location" => self.url = Some(value.trim().to_string()),
            "zipfile_datetime_iso8601" => {
                self.timestamp = parse_catalog_timestamp(value.trim()).ok();
            }
            _ => {}
        }
    }

    fn push_nested(&mut self, field: NestedField, value: String) {
        let value = value.trim().to_string();
        if value.is_empty() {
            return;
        }
        match field {
            NestedField::State => self.states.push(value),
            NestedField::Region => self.regions.push(value),
            NestedField::Cgd => self.coast_guard_districts.push(value),
        }
    }

    fn finish(self) -> Result<Option<Cell>> {
        let (Some(name), Some(url), Some(timestamp)) = (self.name, self.url, self.timestamp) else {
            return Ok(None);
        };
        Ok(Some(Cell {
            name,
            url,
            timestamp,
            states: self.states,
            regions: self.regions,
            coast_guard_districts: self.coast_guard_districts,
        }))
    }
}

fn extract_enc_zip(zip_path: &Path, target_dir: &Path) -> Result<()> {
    fs::create_dir_all(target_dir)?;
    let target_dir = target_dir
        .canonicalize()
        .unwrap_or_else(|_| target_dir.to_path_buf());

    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let entry_path = Path::new(entry.name());
        let components: Vec<_> = entry_path.components().collect();
        if components.is_empty() {
            continue;
        }
        let rel_components: Vec<_> = if components.len() > 1 {
            components[1..].to_vec()
        } else {
            components
        };
        if rel_components.is_empty() {
            continue;
        }

        let mut dest = target_dir.clone();
        for component in &rel_components {
            match component {
                Component::Normal(part) => dest.push(part),
                Component::CurDir => {}
                _ => bail!("zip entry escapes target dir: {}", entry.name()),
            }
        }
        if !dest.starts_with(&target_dir) {
            bail!("zip entry escapes target dir: {}", entry.name());
        }

        if entry.is_dir() || entry.name().ends_with('/') {
            fs::create_dir_all(&dest)?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut outfile = File::create(&dest)?;
        copy(&mut entry, &mut outfile)?;
    }

    Ok(())
}

fn download_cell(chart_dir: &Path, cell: &Cell) -> Result<()> {
    let zip_name = cell
        .url
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| cell.name.as_str());
    let zip_path = chart_dir.join(zip_name);

    log::info!("Downloading {}", cell.name);
    let bytes = fetch_url(&cell.url)?;
    fs::write(&zip_path, bytes)?;

    let extract_result = extract_enc_zip(&zip_path, chart_dir);
    let _ = fs::remove_file(&zip_path);
    extract_result?;

    let cell_dir = chart_dir.join(&cell.name);
    if cell_dir.is_dir() {
        let modified = file_time_from_timestamp(cell.timestamp);
        filetime::set_file_mtime(&cell_dir, modified)?;
    }

    Ok(())
}

fn file_time_from_timestamp(timestamp: i64) -> filetime::FileTime {
    filetime::FileTime::from_unix_time(timestamp, 0)
}

fn find_opencpn() -> Option<PathBuf> {
    for candidate in [
        which_opencpn(),
        Some(PathBuf::from("/Applications/OpenCPN.app/Contents/MacOS/OpenCPN")),
        Some(PathBuf::from("/Applications/OpenCPN.app/Contents/MacOS/opencpn")),
    ]
    .into_iter()
    .flatten()
    {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn which_opencpn() -> Option<PathBuf> {
    Command::new("sh")
        .arg("-c")
        .arg("command -v opencpn")
        .stdout(Stdio::piped())
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
            } else {
                None
            }
        })
}

fn restart_opencpn(rebuild_db: bool) -> Result<()> {
    let Some(opencpn) = find_opencpn() else {
        log::warn!("OpenCPN not found; skipping restart");
        return Ok(());
    };

    log::info!("Requesting running OpenCPN to quit");
    let _ = Command::new(&opencpn)
        .args(["--remote", "-q"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    thread::sleep(Duration::from_secs(3));

    if rebuild_db {
        log::info!("Starting OpenCPN with chart database rebuild (-D)");
    } else {
        log::info!("Starting OpenCPN");
    }

    if cfg!(target_os = "macos") && opencpn.to_string_lossy().contains("OpenCPN.app") {
        let mut command = Command::new("open");
        command.arg("-a").arg("OpenCPN");
        if rebuild_db {
            command.args(["--args", "-D"]);
        }
        command.spawn()?;
    } else {
        let mut command = Command::new(&opencpn);
        if rebuild_db {
            command.arg("-D");
        }
        command.spawn()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_matches_state_or_region() {
        let filters = Filters {
            states: HashSet::from(["CA".to_string()]),
            regions: HashSet::new(),
            coast_guard_districts: HashSet::new(),
        };
        let cell = Cell {
            name: "US5CA01M".to_string(),
            url: String::new(),
            timestamp: 0,
            states: vec!["CA".to_string()],
            regions: vec![],
            coast_guard_districts: vec![],
        };
        assert!(filters.matches(&cell));

        let other = Cell {
            states: vec!["FL".to_string()],
            ..cell.clone()
        };
        assert!(!filters.matches(&other));
    }
}
