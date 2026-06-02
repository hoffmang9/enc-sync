//! Cron-aware logging: routine progress can drop to debug with `--cron`.

pub fn routine(cron: bool, message: &str) {
    if cron {
        log::debug!("{message}");
    } else {
        log::info!("{message}");
    }
}

pub fn important(message: &str) {
    log::info!("{message}");
}
