// Copyright 2026 Gene Hoffman
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Sync NOAA ENC charts for OpenCPN.
//!
//! NOAA publishes ENC updates every weekday evening; run after that (most days
//! the binary exits quickly when nothing changed). Example cron:
//!   0 23 * * 1-5 enc-sync --config ~/.enc-sync/config.toml --cron \
//!     >>/tmp/enc-sync.log 2>&1
//!
//! Chart folders follow OpenCPN Chart Downloader defaults under
//! `{chart_dir}/ENC/` (states, regions, Coast Guard districts, national,
//! and US Army Corps inland catalogs).

mod catalog;
mod charts;
mod config;
mod opencpn;
mod source_kind;
mod source_norm;
#[allow(dead_code)] // `source_kind_rust_expr_for_folder` is used by build.rs
#[path = "source_taxonomy.rs"]
mod source_taxonomy;
mod sources;
#[cfg(any(test, feature = "test-helpers"))]
pub mod test_env;

use anyhow::{bail, Result};

pub use catalog::{parse_catalog, Cell};
pub use config::{home_dir, load_config, Config};
pub use source_kind::SourceKind;
pub use sources::ChartSource;

use charts::{
    cell_key, download_catalog, download_cell, load_update_data, needs_update, save_update_data,
};
use config::prepare_chart_dir;
use opencpn::restart_opencpn;
use sources::{config_minimum_summary, enc_root, select_sources};

/// Controls whether enc-sync downloads chart cells or only refreshes catalogs.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunOptions {
    /// Download catalogs for selected sources, but do not download chart cells
    /// or restart OpenCPN.
    pub catalog_only: bool,
    /// Reduce routine progress logs for cron; errors and download activity stay at info.
    pub cron: bool,
}

/// Log at info by default; in cron mode, `detail` messages drop to debug.
macro_rules! run_log {
    ($options:expr, info, $($arg:tt)*) => {
        log::info!($($arg)*)
    };
    ($options:expr, detail, $($arg:tt)*) => {
        if $options.cron {
            log::debug!($($arg)*);
        } else {
            log::info!($($arg)*);
        }
    };
}

pub fn run(config: &Config) -> Result<()> {
    run_with_options(config, RunOptions::default())
}

pub fn run_with_options(config: &Config, options: RunOptions) -> Result<()> {
    let enc_root = enc_root(config);
    prepare_chart_dir(&enc_root)?;
    let sources = select_sources(config)?;

    if let Some(summary) = config_minimum_summary(config) {
        run_log!(options, info, "Config minimum: {summary}");
    }
    run_log!(
        options,
        info,
        "Syncing {} chart source(s): {}",
        sources.len(),
        sources
            .iter()
            .map(|source| source.folder)
            .collect::<Vec<_>>()
            .join(", ")
    );

    let mut failed = Vec::new();
    let mut updated = 0usize;

    for source in &sources {
        run_log!(
            options,
            detail,
            "Source {} → {}",
            source.name,
            source.folder
        );
        match sync_source(config, &enc_root, source, options) {
            Ok(count) => updated += count,
            Err(error) => {
                log::error!("Failed syncing {}: {:#}", source.folder, error);
                failed.push(source.folder.to_string());
            }
        }
    }

    if options.catalog_only {
        run_log!(options, detail, "Catalog-only mode; skipping OpenCPN restart");
        return finalize(failed);
    }

    if updated > 0 && config.restart_opencpn {
        restart_opencpn(config.rebuild_chart_db)?;
    } else if updated == 0 {
        run_log!(options, info, "No chart files changed; leaving OpenCPN running");
    }

    finalize(failed)
}

fn sync_source(
    config: &Config,
    enc_root: &std::path::Path,
    source: &ChartSource,
    options: RunOptions,
) -> Result<usize> {
    let chart_dir = source.chart_dir(enc_root);
    prepare_chart_dir(&chart_dir)?;

    let catalog_path = download_catalog(
        &source.resolve_catalog_url(config),
        &chart_dir,
        source.catalog_filename,
        options.cron,
    )?;

    if options.catalog_only {
        return Ok(0);
    }

    let cells = parse_catalog(&catalog_path)?;
    run_log!(
        options,
        detail,
        "{} lists {} chart cells",
        source.catalog_filename,
        cells.len()
    );

    let mut update_data = load_update_data(&chart_dir)?;
    let pending: Vec<_> = cells
        .into_iter()
        .filter(|cell| needs_update(&chart_dir, cell, &update_data))
        .collect();

    if pending.is_empty() {
        run_log!(
            options,
            detail,
            "All cells up to date in {}",
            chart_dir.display()
        );
        return Ok(0);
    }

    let total = pending.len();
    log::info!("Downloading {total} updated or new cells into {}", source.folder);
    let mut updated = 0usize;
    for (index, cell) in pending.into_iter().enumerate() {
        log::info!(
            "[{}] Downloading {} ({} of {total})",
            source.folder,
            cell.name,
            index + 1
        );
        download_cell(&chart_dir, &cell)?;
        update_data.insert(cell_key(&cell.name), cell.timestamp);
        updated += 1;
    }
    save_update_data(&chart_dir, &update_data)?;
    Ok(updated)
}

fn finalize(failed: Vec<String>) -> Result<()> {
    if failed.is_empty() {
        Ok(())
    } else {
        bail!("{} chart source(s) failed: {}", failed.len(), failed.join(", "))
    }
}
