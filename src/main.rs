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

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use enc_sync::{home_dir, load_config, run};

const HOME_CONFIG_REL: &str = ".enc-sync/config.toml";

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config_path = parse_config_path()?;
    log::info!("Using config {}", config_path.display());
    let config = load_config(&config_path)?;
    run(&config)
}

fn discover_config_path() -> Option<PathBuf> {
    let local = PathBuf::from("enc-sync.toml");
    if local.is_file() {
        return Some(local);
    }
    home_dir()
        .map(|home| home.join(HOME_CONFIG_REL))
        .filter(|path| path.is_file())
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
    if let Some(path) = discover_config_path() {
        return Ok(path);
    }
    bail!(
        "No config file found. Pass --config /path/to/enc-sync.toml\n\n{}",
        help_text()
    )
}

fn print_help() {
    print!("{}", help_text());
}

fn help_text() -> &'static str {
    r#"enc-sync -- Sync NOAA ENC charts for OpenCPN

Usage:
  enc-sync --config /path/to/enc-sync.toml

If --config is omitted, looks for config in this order:
  1. enc-sync.toml in the current directory
  2. ~/.enc-sync/config.toml

See enc-sync.example.toml for configuration options.
"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn discover_config_path_prefers_local_enc_sync_toml() {
        let dir = TempDir::new().unwrap();
        let local = dir.path().join("enc-sync.toml");
        fs::write(&local, "chart_dir = \"/tmp/charts\"\n").unwrap();

        let saved_home = std::env::var_os("HOME");
        #[cfg(windows)]
        let saved_profile = std::env::var_os("USERPROFILE");
        let home = TempDir::new().unwrap();
        std::env::set_var("HOME", home.path());
        #[cfg(windows)]
        std::env::set_var("USERPROFILE", home.path());
        let home_config = home.path().join(HOME_CONFIG_REL);
        fs::create_dir_all(home_config.parent().unwrap()).unwrap();
        fs::write(&home_config, "chart_dir = \"/other\"\n").unwrap();

        let saved_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let discovered = discover_config_path();
        std::env::set_current_dir(saved_cwd).unwrap();
        if let Some(prev) = saved_home {
            std::env::set_var("HOME", prev);
        } else {
            std::env::remove_var("HOME");
        }
        #[cfg(windows)]
        match saved_profile {
            Some(prev) => std::env::set_var("USERPROFILE", prev),
            None => std::env::remove_var("USERPROFILE"),
        }

        assert_eq!(discovered, Some(PathBuf::from("enc-sync.toml")));
    }
}
