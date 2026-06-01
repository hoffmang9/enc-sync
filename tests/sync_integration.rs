mod common;

use std::path::Path;

use common::{build_cell_zip, MockHttpServer};
use enc_sync::{load_config, run};
use tempfile::TempDir;

const CA_TIMESTAMP: i64 = 1_717_200_000;

fn write_config(config_path: &Path, chart_dir: &str, catalog_url: &str) {
    let text = format!(
        r#"
chart_dir = "{chart_dir}"
catalog_url = "{catalog_url}"
states = ["CA"]
restart_opencpn = false
rebuild_chart_db = false
"#
    );
    std::fs::write(config_path, text).unwrap();
}

#[test]
fn sync_downloads_filtered_cells_and_skips_when_up_to_date() {
    let chart_home = TempDir::new().unwrap();
    let chart_dir = chart_home.path().to_path_buf();
    let server = MockHttpServer::start(
        build_cell_zip("US5CA01M", b"ca-chart-bytes"),
        build_cell_zip("US5FL01M", b"fl-chart-bytes"),
    );
    let config_path = chart_home.path().join("enc-sync.toml");
    write_config(
        &config_path,
        &chart_dir.display().to_string(),
        &format!("{}/ENCProdCat.xml", server.base_url),
    );
    let config = load_config(&config_path).unwrap();

    run(&config).expect("first sync should download the CA cell");

    let chart_file = chart_dir.join("US5CA01M/US5CA01M.000");
    assert!(chart_file.is_file(), "expected extracted chart file");
    assert_eq!(
        std::fs::read(&chart_file).unwrap(),
        b"ca-chart-bytes",
        "extracted chart contents should match the zip payload"
    );
    assert!(
        !chart_dir.join("US5FL01M").exists(),
        "FL cell should be filtered out"
    );

    let dat = std::fs::read_to_string(chart_dir.join("chartdldr_pi.dat")).unwrap();
    assert!(dat.contains(&format!("us5ca01m {CA_TIMESTAMP}")));
    assert!(!dat.contains("us5fl01m"));

    assert_eq!(
        server
            .zip_downloads
            .load(std::sync::atomic::Ordering::SeqCst),
        1,
        "only the CA zip should have been downloaded"
    );

    run(&config).expect("second sync should succeed with nothing to do");

    assert_eq!(
        server
            .zip_downloads
            .load(std::sync::atomic::Ordering::SeqCst),
        1,
        "second sync should not re-download an up-to-date cell"
    );
}

#[cfg(not(windows))]
#[test]
fn sync_uses_tilde_chart_dir_from_home_config() {
    let home = TempDir::new().unwrap();
    let _home_guard = common::TestHome::set(home.path());

    let config_dir = home.path().join(".enc-sync");
    std::fs::create_dir_all(&config_dir).unwrap();
    let chart_dir = home.path().join("Charts/ENC/US");
    std::fs::create_dir_all(&chart_dir).unwrap();

    let server = MockHttpServer::start(
        build_cell_zip("US5CA01M", b"tilde-home-chart"),
        build_cell_zip("US5FL01M", b"unused"),
    );
    let config_path = config_dir.join("config.toml");
    write_config(
        &config_path,
        "~/Charts/ENC/US",
        &format!("{}/ENCProdCat.xml", server.base_url),
    );

    let config = load_config(&config_path).expect("tilde chart_dir should expand");
    assert_eq!(config.chart_dir, chart_dir);

    run(&config).expect("sync should succeed with expanded chart dir");

    let chart_file = chart_dir.join("US5CA01M/US5CA01M.000");
    assert!(chart_file.is_file());
    assert_eq!(std::fs::read(chart_file).unwrap(), b"tilde-home-chart");
}
