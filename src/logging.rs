//! Cron-aware logging: routine progress can drop to debug with `--cron`.

use crate::RunOptions;

#[derive(Debug, Clone, Copy, Default)]
pub struct LogOptions {
    pub cron: bool,
}

impl From<RunOptions> for LogOptions {
    fn from(options: RunOptions) -> Self {
        Self { cron: options.cron }
    }
}

pub fn routine(options: LogOptions, message: &str) {
    if options.cron {
        log::debug!("{message}");
    } else {
        log::info!("{message}");
    }
}
