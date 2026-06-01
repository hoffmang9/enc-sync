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
//!   0 23 * * 1-5 enc-sync --config ~/.enc-sync/config.toml \
//!     >>/tmp/enc-sync.log 2>&1

mod catalog;
mod charts;
mod config;
mod filters;
mod opencpn;
#[cfg(any(test, feature = "test-helpers"))]
pub mod test_env;

use anyhow::{bail, Result};

pub use catalog::Cell;
pub use config::{home_dir, load_config, Config};

use catalog::parse_catalog;
use charts::{
    cell_key, download_catalog, download_cell, load_update_data, needs_update, save_update_data,
};
use filters::Filters;
use opencpn::restart_opencpn;

pub fn run(config: &Config) -> Result<()> {
    config::prepare_chart_dir(&config.chart_dir)?;
    let filters = Filters::from_config(config);

    if filters.is_active() {
        let (states, regions, cgd) = filters.active_counts();
        log::info!("Filters active: {states} state(s), {regions} region(s), {cgd} CG district(s)");
    } else {
        log::info!("No area filters configured; all catalog cells are eligible");
    }

    let catalog_path = download_catalog(config)?;
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
        let total = pending.len();
        log::info!("Downloading {total} updated or new cells");
        for (index, cell) in pending.into_iter().enumerate() {
            log::info!("Downloading {} ({} of {total})", cell.name, index + 1);
            let result = download_cell(&config.chart_dir, &cell);
            match result {
                Ok(()) => {
                    update_data.insert(cell_key(&cell.name), cell.timestamp);
                    updated += 1;
                }
                Err(error) => {
                    log::error!(
                        "Failed to update {} ({} of {total}): {:#}",
                        cell.name,
                        index + 1,
                        error
                    );
                    failed.push(cell.name);
                }
            }
        }
        if updated > 0 {
            save_update_data(&config.chart_dir, &update_data)?;
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
