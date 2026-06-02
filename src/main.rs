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
use enc_sync::{load_config, run_with_options, RunOptions};

const HOME_CONFIG_REL: &str = ".enc-sync/config.toml";

#[cfg(test)]
#[allow(dead_code)]
#[path = "test_env.rs"]
mod test_env;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let (config_path, options) = parse_args()?;
    if options.cron {
        log::debug!("Using config {}", config_path.display());
    } else {
        log::info!("Using config {}", config_path.display());
    }
    let config = load_config(&config_path)?;
    run_with_options(&config, options)
}

fn discover_config_path() -> Option<PathBuf> {
    let local = PathBuf::from("enc-sync.toml");
    if local.is_file() {
        return Some(local);
    }
    enc_sync::home_dir()
        .map(|home| home.join(HOME_CONFIG_REL))
        .filter(|path| path.is_file())
}

fn parse_args() -> Result<(PathBuf, RunOptions)> {
    let args: Vec<String> = std::env::args().collect();
    let mut config_path = None;
    let mut options = RunOptions::default();

    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                config_path = Some(
                    args.get(index + 1)
                        .map(PathBuf::from)
                        .context("--config requires a path argument")?,
                );
                index += 2;
            }
            "--catalog-only" => {
                options.catalog_only = true;
                index += 1;
            }
            "--cron" => {
                options.cron = true;
                index += 1;
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => bail!("unknown argument: {other}\n\n{}", help_text()),
        }
    }

    let config_path = config_path
        .or_else(discover_config_path)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No config file found. Pass --config /path/to/enc-sync.toml\n\n{}",
                help_text()
            )
        })?;

    Ok((config_path, options))
}

fn print_help() {
    print!("{}", help_text());
}

fn help_text() -> &'static str {
    r#"enc-sync -- Sync NOAA ENC charts for OpenCPN

Usage:
  enc-sync [--config /path/to/enc-sync.toml] [--catalog-only] [--cron]

Options:
  --config PATH    Config file (default: ./enc-sync.toml, then ~/.enc-sync/config.toml)
  --catalog-only   Download latest catalogs only; do not download chart cells or restart OpenCPN
  --cron           Quieter routine progress logs for cron (errors and downloads stay at info)

Chart folders mirror OpenCPN Chart Downloader defaults under chart_dir/ENC/
(US_CA, US_OR, US_REGION14, US_CGD13, US, US_INLAND, …).

See enc-sync.example.toml and README.md for configuration options.
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

        let home = TempDir::new().unwrap();
        let home_config = home.path().join(HOME_CONFIG_REL);
        fs::create_dir_all(home_config.parent().unwrap()).unwrap();
        fs::write(&home_config, "chart_dir = \"/other\"\n").unwrap();

        let _guard = crate::test_env::EnvGuard::override_home_dirs_and_cwd(home.path(), dir.path());
        let discovered = discover_config_path();

        assert_eq!(discovered, Some(PathBuf::from("enc-sync.toml")));
    }
}
