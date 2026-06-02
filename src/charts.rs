use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{copy, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use zip::read::ZipArchive;

use crate::catalog::Cell;
use crate::logging::routine;
use crate::RunOptions;

pub(crate) const UPDATE_DATA_FILENAME: &str = "chartdldr_pi.dat";
const USER_AGENT: &str = "enc-sync/0.1";
const ENC_ROOT: &str = "ENC_ROOT";

pub(crate) fn cell_key(cell_name: &str) -> String {
    cell_name.to_ascii_lowercase()
}

pub(crate) fn load_update_data(chart_dir: &Path) -> Result<BTreeMap<String, i64>> {
    let path = chart_dir.join(UPDATE_DATA_FILENAME);
    let mut data = BTreeMap::new();
    if !path.is_file() {
        return Ok(data);
    }
    for line in fs::read_to_string(&path)?.lines() {
        let mut parts = line.split_whitespace();
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            if let Ok(timestamp) = value.parse::<i64>() {
                data.insert(cell_key(key), timestamp);
            }
        }
    }
    Ok(data)
}

pub(crate) fn save_update_data(chart_dir: &Path, data: &BTreeMap<String, i64>) -> Result<()> {
    let path = chart_dir.join(UPDATE_DATA_FILENAME);
    write_atomically(&path, |file| {
        for (key, value) in data {
            writeln!(file, "{key} {value}")?;
        }
        Ok(())
    })
}

pub(crate) fn write_atomically(
    path: &Path,
    write: impl FnOnce(&mut File) -> Result<()>,
) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("creating temporary file near {}", path.display()))?;
    write(tmp.as_file_mut())?;
    tmp.as_file_mut()
        .sync_all()
        .with_context(|| format!("flushing temporary file for {}", path.display()))?;
    tmp.persist(path)
        .with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

pub(crate) fn local_cell_mtime(chart_dir: &Path, cell_name: &str) -> Option<i64> {
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

    let lower = cell_key(cell_name);
    fs::read_dir(chart_dir)
        .ok()?
        .filter_map(Result::ok)
        .find_map(|entry| {
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

pub(crate) fn exists_locally(
    chart_dir: &Path,
    cell_name: &str,
    update_data: &BTreeMap<String, i64>,
) -> bool {
    if update_data.contains_key(&cell_key(cell_name)) {
        return true;
    }
    local_cell_mtime(chart_dir, cell_name).is_some()
}

pub(crate) fn needs_update(
    chart_dir: &Path,
    cell: &Cell,
    update_data: &BTreeMap<String, i64>,
) -> bool {
    if !exists_locally(chart_dir, &cell.name, update_data) {
        return true;
    }
    let key = cell_key(&cell.name);
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

pub(crate) fn download_catalog(
    catalog_url: &str,
    chart_dir: &Path,
    catalog_filename: &str,
    options: RunOptions,
) -> Result<PathBuf> {
    let catalog_path = chart_dir.join(catalog_filename);

    routine(options, &format!("Downloading catalog {catalog_url}"));
    let catalog_url = catalog_url.to_string();
    write_atomically(&catalog_path, |file| {
        file.write_all(&fetch_url(&catalog_url)?)?;
        Ok(())
    })?;
    routine(
        options,
        &format!("Catalog saved to {}", catalog_path.display()),
    );
    Ok(catalog_path)
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
        let rel_components = zip_entry_rel_components(entry.name())?;
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

fn zip_entry_rel_components(entry_name: &str) -> Result<Vec<Component<'_>>> {
    let components: Vec<_> = Path::new(entry_name).components().collect();
    for component in &components {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => bail!("zip entry escapes target dir: {entry_name}"),
        }
    }
    let rel = if matches!(
        components.first(),
        Some(Component::Normal(name)) if name.to_str() == Some(ENC_ROOT)
    ) {
        components[1..].to_vec()
    } else {
        components
    };
    Ok(rel)
}

pub(crate) fn download_cell(chart_dir: &Path, cell: &Cell) -> Result<()> {
    let zip_name = cell
        .url
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(cell.name.as_str());
    let zip_path = chart_dir.join(zip_name);

    let bytes = fetch_url(&cell.url)?;
    fs::write(&zip_path, bytes)?;

    let extract_result = extract_enc_zip(&zip_path, chart_dir);
    let _ = fs::remove_file(&zip_path);
    extract_result?;

    let cell_dir = chart_dir.join(&cell.name);
    if cell_dir.is_dir() {
        filetime::set_file_mtime(&cell_dir, file_time_from_timestamp(cell.timestamp))?;
    }

    Ok(())
}

fn file_time_from_timestamp(timestamp: i64) -> filetime::FileTime {
    filetime::FileTime::from_unix_time(timestamp, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Cell;
    use tempfile::TempDir;

    fn sample_cell(name: &str) -> Cell {
        Cell {
            name: name.to_string(),
            url: format!("https://example.test/{name}.zip"),
            timestamp: 1_700_000_000,
        }
    }

    #[test]
    fn update_data_roundtrip_and_missing_file() {
        let dir = TempDir::new().unwrap();
        let chart_dir = dir.path();

        let empty = load_update_data(chart_dir).unwrap();
        assert!(empty.is_empty());

        let mut data = BTreeMap::new();
        data.insert("us5ca01m".to_string(), 1_700_000_000);
        data.insert("us5or02m".to_string(), 1_700_000_001);
        save_update_data(chart_dir, &data).unwrap();

        let loaded = load_update_data(chart_dir).unwrap();
        assert_eq!(loaded, data);

        let dat_path = chart_dir.join(UPDATE_DATA_FILENAME);
        let text = fs::read_to_string(dat_path).unwrap();
        assert!(text.contains("us5ca01m 1700000000"));
    }

    #[test]
    fn needs_update_when_cell_missing_or_stale() {
        let dir = TempDir::new().unwrap();
        let chart_dir = dir.path();
        let cell = sample_cell("US5CA01M");

        let update_data = BTreeMap::new();
        assert!(needs_update(chart_dir, &cell, &update_data));

        let mut update_data = BTreeMap::new();
        update_data.insert("us5ca01m".to_string(), cell.timestamp - 1);
        assert!(needs_update(chart_dir, &cell, &update_data));

        update_data.insert("us5ca01m".to_string(), cell.timestamp);
        assert!(!needs_update(chart_dir, &cell, &update_data));

        update_data.insert("us5ca01m".to_string(), cell.timestamp + 1);
        assert!(!needs_update(chart_dir, &cell, &update_data));
    }

    #[test]
    fn exists_locally_from_update_data_or_directory() {
        let dir = TempDir::new().unwrap();
        let chart_dir = dir.path();

        let update_data = BTreeMap::new();
        assert!(!exists_locally(chart_dir, "US5CA01M", &update_data));

        let mut update_data = BTreeMap::new();
        update_data.insert("us5ca01m".to_string(), 1);
        assert!(exists_locally(chart_dir, "US5CA01M", &update_data));

        update_data.clear();
        fs::create_dir_all(chart_dir.join("us5ca01m")).unwrap();
        assert!(exists_locally(chart_dir, "US5CA01M", &update_data));
    }

    #[test]
    fn local_cell_mtime_finds_case_insensitive_directory() {
        let dir = TempDir::new().unwrap();
        let chart_dir = dir.path();
        let cell_dir = chart_dir.join("us5ca01m");
        fs::create_dir_all(&cell_dir).unwrap();

        let expected = 1_700_000_000_i64;
        filetime::set_file_mtime(&cell_dir, file_time_from_timestamp(expected)).unwrap();

        let mtime = local_cell_mtime(chart_dir, "US5CA01M").unwrap();
        assert_eq!(mtime, expected);
    }

    #[test]
    fn file_time_from_timestamp_roundtrip() {
        let timestamp = 1_700_000_000_i64;
        let ft = file_time_from_timestamp(timestamp);
        assert_eq!(ft.unix_seconds(), timestamp);
    }

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        use zip::write::SimpleFileOptions;
        use zip::ZipWriter;

        let file = File::create(path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        for (name, contents) in entries {
            zip.start_file(*name, options).unwrap();
            zip.write_all(contents).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn extract_enc_zip_strips_enc_root_prefix() {
        let dir = TempDir::new().unwrap();
        let zip_path = dir.path().join("cell.zip");
        let chart_dir = dir.path().join("charts");
        write_zip(
            &zip_path,
            &[
                ("ENC_ROOT/US5CA01M/US5CA01M.000", b"chart-data"),
                ("ENC_ROOT/US5CA01M/INFO/", &[]),
            ],
        );

        extract_enc_zip(&zip_path, &chart_dir).unwrap();

        let chart_file = chart_dir.join("US5CA01M/US5CA01M.000");
        assert!(chart_file.is_file());
        assert_eq!(fs::read(&chart_file).unwrap(), b"chart-data");
    }

    #[test]
    fn extract_enc_zip_rejects_path_traversal() {
        let dir = TempDir::new().unwrap();
        let zip_path = dir.path().join("bad.zip");
        let chart_dir = dir.path().join("charts");

        for entry in [
            ("ENC_ROOT/../../outside.txt", b"nope" as &[u8]),
            ("../escape.txt", b"nope"),
        ] {
            write_zip(&zip_path, &[entry]);
            let error = extract_enc_zip(&zip_path, &chart_dir).unwrap_err();
            assert!(
                error.to_string().contains("escapes target dir"),
                "expected rejection for {}, got: {error:#}",
                entry.0
            );
        }
    }
}
