//! Cron-aware logging: routine progress can drop to debug with `--cron`.

use crate::RunOptions;

pub fn routine(options: RunOptions, message: &str) {
    if options.cron {
        log::debug!("{message}");
    } else {
        log::info!("{message}");
    }
}
