use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::Result;

pub fn restart_opencpn(rebuild_db: bool) -> Result<()> {
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

fn find_opencpn() -> Option<PathBuf> {
    [
        which_opencpn(),
        Some(PathBuf::from(
            "/Applications/OpenCPN.app/Contents/MacOS/OpenCPN",
        )),
        Some(PathBuf::from(
            "/Applications/OpenCPN.app/Contents/MacOS/opencpn",
        )),
    ]
    .into_iter()
    .flatten()
    .find(|candidate| candidate.is_file())
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
                Some(PathBuf::from(
                    String::from_utf8_lossy(&output.stdout).trim(),
                ))
            } else {
                None
            }
        })
}
