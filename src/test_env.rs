//! RAII helpers for temporarily overriding process environment variables in tests.

use std::ffi::{OsStr, OsString};
use std::path::Path;

fn set_env(key: &str, value: impl AsRef<OsStr>) {
    // SAFETY: only used from single-threaded test code that restores on drop.
    unsafe { std::env::set_var(key, value) }
}

fn remove_env(key: &str) {
    // SAFETY: only used from single-threaded test code that restores on drop.
    unsafe { std::env::remove_var(key) }
}

fn restore_env(key: &str, value: Option<OsString>) {
    match value {
        Some(v) => set_env(key, v),
        None => remove_env(key),
    }
}

/// Saves and restores environment variables for the guard's lifetime.
pub struct EnvGuard {
    saved: Vec<(String, Option<OsString>)>,
}

impl EnvGuard {
    fn save_and_set(
        saved: &mut Vec<(String, Option<OsString>)>,
        key: &str,
        value: impl AsRef<OsStr>,
    ) {
        saved.push((key.to_string(), std::env::var_os(key)));
        set_env(key, value);
    }

    fn save_and_remove(saved: &mut Vec<(String, Option<OsString>)>, key: &str) {
        saved.push((key.to_string(), std::env::var_os(key)));
        remove_env(key);
    }

    /// Override `HOME`, and `USERPROFILE` on Windows, for config discovery tests.
    pub fn override_home_dirs(path: &Path) -> Self {
        let mut saved = Vec::new();
        Self::save_and_set(&mut saved, "HOME", path);
        #[cfg(windows)]
        Self::save_and_set(&mut saved, "USERPROFILE", path);
        Self { saved }
    }

    /// Unix-only: override `HOME` for tilde expansion tests.
    #[cfg(not(windows))]
    pub fn override_home(path: &Path) -> Self {
        let mut saved = Vec::new();
        Self::save_and_set(&mut saved, "HOME", path);
        Self { saved }
    }

    /// Clear home-related env vars to simulate a missing home directory.
    pub fn clear_home_dirs() -> Self {
        let mut saved = Vec::new();
        Self::save_and_remove(&mut saved, "HOME");
        #[cfg(windows)]
        Self::save_and_remove(&mut saved, "USERPROFILE");
        Self { saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.saved.drain(..) {
            restore_env(&key, value);
        }
    }
}

#[cfg(not(windows))]
impl EnvGuard {
    pub fn set(path: &Path) -> Self {
        Self::override_home(path)
    }
}

#[cfg(not(windows))]
pub type TestHome = EnvGuard;
