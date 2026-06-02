mod common;

use std::path::Path;

use common::{build_cell_zip, MockHttpServer};
use enc_sync::{load_config, run, run_with_options, RunOptions};
use tempfile::TempDir;

const CA_TIMESTAMP: i64 = 1_717_200_000;

fn write_config(config_path: &Path, chart_dir: &str, catalog_base_url: &str) {
    let chart_dir = common::toml_path(Path::new(chart_dir));
    let text = format!(
        r#"
chart_dir = "{chart_dir}"
catalog_base_url = "{catalog_base_url}"
states = ["CA"]
restart_opencpn = false
rebuild_chart_db = false
"#
    );
    std::fs::write(config_path, text).unwrap();
}

#[test]
fn sync_downloads_state_folder_and_skips_when_up_to_date() {
    let chart_home = TempDir::new().unwrap();
    let chart_base = chart_home.path().to_path_buf();
    let server = MockHttpServer::start(
        build_cell_zip("US5CA01M", b"ca-chart-bytes"),
        build_cell_zip("US5FL01M", b"fl-chart-bytes"),
    );
    let config_path = chart_home.path().join("enc-sync.toml");
    write_config(
        &config_path,
        &chart_base.display().to_string(),
        &server.base_url,
    );
    let config = load_config(&config_path).unwrap();

    run(&config).expect("first sync should download the CA cell");

    let chart_file = chart_base.join("ENC/US_CA/US5CA01M/US5CA01M.000");
    assert!(chart_file.is_file(), "expected extracted chart file");
    assert_eq!(std::fs::read(&chart_file).unwrap(), b"ca-chart-bytes");
    assert!(!chart_base.join("ENC/US_FL").exists());

    let dat = std::fs::read_to_string(chart_base.join("ENC/US_CA/chartdldr_pi.dat")).unwrap();
    assert!(dat.contains(&format!("us5ca01m {CA_TIMESTAMP}")));
    assert!(chart_base.join("ENC/US_CA/CA_ENCProdCat.xml").is_file());

    assert_eq!(
        server
            .zip_downloads
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );

    run(&config).expect("second sync should succeed with nothing to do");
    assert_eq!(
        server
            .zip_downloads
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}

#[cfg(not(windows))]
#[test]
fn sync_uses_tilde_chart_dir_from_home_config() {
    let home = TempDir::new().unwrap();
    let _home_guard = common::TestHome::set(home.path());

    let config_dir = home.path().join(".enc-sync");
    std::fs::create_dir_all(&config_dir).unwrap();
    let chart_base = home.path().join("Documents/Charts");

    let server = MockHttpServer::start(
        build_cell_zip("US5CA01M", b"tilde-home-chart"),
        build_cell_zip("US5FL01M", b"unused"),
    );
    let config_path = config_dir.join("config.toml");
    write_config(&config_path, "~/Documents/Charts", &server.base_url);

    let config = load_config(&config_path).expect("tilde chart_dir should expand");
    assert_eq!(config.chart_dir, chart_base);

    run(&config).expect("sync should succeed with expanded chart dir");

    let chart_file = chart_base.join("ENC/US_CA/US5CA01M/US5CA01M.000");
    assert!(chart_file.is_file());
    assert_eq!(std::fs::read(chart_file).unwrap(), b"tilde-home-chart");
}

#[test]
fn catalog_only_downloads_catalog_without_cells() {
    let chart_home = TempDir::new().unwrap();
    let chart_base = chart_home.path().to_path_buf();
    let server = MockHttpServer::start(
        build_cell_zip("US5CA01M", b"unused"),
        build_cell_zip("US5FL01M", b"unused"),
    );
    let config_path = chart_home.path().join("enc-sync.toml");
    write_config(
        &config_path,
        &chart_base.display().to_string(),
        &server.base_url,
    );
    let config = load_config(&config_path).unwrap();

    run_with_options(
        &config,
        RunOptions {
            catalog_only: true,
            ..Default::default()
        },
    )
    .expect("catalog-only sync");

    assert!(chart_base.join("ENC/US_CA/CA_ENCProdCat.xml").is_file());
    assert!(!chart_base.join("ENC/US_CA/US5CA01M").exists());
    assert_eq!(
        server
            .zip_downloads
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
}

#[test]
fn catalog_only_overwrites_stale_local_catalog() {
    let chart_home = TempDir::new().unwrap();
    let chart_base = chart_home.path().to_path_buf();
    let server = MockHttpServer::start(
        build_cell_zip("US5CA01M", b"unused"),
        build_cell_zip("US5FL01M", b"unused"),
    );
    let config_path = chart_home.path().join("enc-sync.toml");
    write_config(
        &config_path,
        &chart_base.display().to_string(),
        &server.base_url,
    );
    let config = load_config(&config_path).unwrap();

    let catalog_path = chart_base.join("ENC/US_CA/CA_ENCProdCat.xml");
    std::fs::create_dir_all(catalog_path.parent().unwrap()).unwrap();
    std::fs::write(&catalog_path, "<catalog>stale</catalog>").unwrap();

    run_with_options(
        &config,
        RunOptions {
            catalog_only: true,
            ..Default::default()
        },
    )
    .expect("catalog-only sync");

    let catalog = std::fs::read_to_string(&catalog_path).unwrap();
    assert!(
        catalog.contains("US5CA01M"),
        "expected fresh catalog from server"
    );
    assert!(!catalog.contains("stale"));
}

#[test]
fn sync_continues_after_cell_failure_and_saves_partial_update_data() {
    let chart_home = TempDir::new().unwrap();
    let chart_base = chart_home.path().to_path_buf();
    let server = MockHttpServer::start_with_second_cell_failing(build_cell_zip(
        "US5CA01M",
        b"partial-success-bytes",
    ));
    let config_path = chart_home.path().join("enc-sync.toml");
    write_config(
        &config_path,
        &chart_base.display().to_string(),
        &server.base_url,
    );
    let config = load_config(&config_path).unwrap();

    let error = run(&config).unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains("chart source(s) failed: US_CA"),
        "unexpected error: {message}"
    );

    let chart_file = chart_base.join("ENC/US_CA/US5CA01M/US5CA01M.000");
    assert!(chart_file.is_file(), "expected first cell to download");
    assert_eq!(
        std::fs::read(&chart_file).unwrap(),
        b"partial-success-bytes"
    );
    assert!(!chart_base.join("ENC/US_CA/US5CA02M").exists());

    let dat = std::fs::read_to_string(chart_base.join("ENC/US_CA/chartdldr_pi.dat")).unwrap();
    assert!(dat.contains("us5ca01m 1717200000"));
    assert!(!dat.contains("us5ca02m"));

    assert_eq!(
        server
            .zip_downloads
            .load(std::sync::atomic::Ordering::SeqCst),
        2
    );
}
