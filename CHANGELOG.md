# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Multi-folder layout mirroring OpenCPN Chart Downloader defaults under
  `{chart_dir}/ENC/` (states, regions, Coast Guard districts, national `US`,
  and US Army Corps inland catalogs).
- Embedded OpenCPN `chart_sources.xml` definitions for NOAA ENC and ACE inland
  catalogs; folder discovery when no filters are configured.
- Config flags `all_enc` and `inland`; optional `catalog_base_url` for tests or
  mirrors.
- IENC (US Army Corps) catalog parsing support.
- `--catalog-only` CLI mode refreshes catalogs without downloading cells.

### Changed

- **Breaking:** `chart_dir` is now OpenCPN's base chart directory (parent of
  `ENC/`), not a single catalog folder. State/region/CGD filters select which
  OpenCPN chart folders to sync instead of filtering a national catalog in place.
- Removed single-folder filtered `ENCProdCat.xml` / `ENCProdCat.upstream.xml`
  behavior and `--catalog-only --local`.

### Fixed

- Chart Downloader compatibility: each source uses its own scoped NOAA catalog
  (e.g. `CA_ENCProdCat.xml` in `ENC/US_CA`), so **Update** works without
  refiltering.

## [1.0.0-rc1] - 2026-06-01

Initial public release of **enc-sync**, a cron-friendly tool that keeps NOAA ENC
charts current for [OpenCPN](https://opencpn.org/) in a Chart Downloader-compatible
layout.

### Added

- Standalone `enc-sync` binary that downloads NOAA `ENCProdCat.xml`, compares cell
  timestamps against `chartdldr_pi.dat` and on-disk cells, and extracts ENC zips
  with the same `ENC_ROOT/` strip used by the Chart Downloader plugin.
- Optional area filters by state, NOAA region, and Coast Guard district (a cell is
  updated when it matches any configured non-empty list).
- Optional OpenCPN restart after successful updates, with chart database rebuild
  (`-D`) when configured.
- Library crate (`enc_sync`) with modules for catalog parsing, chart download and
  local state, configuration, filters, and OpenCPN integration.
- Configuration discovery: `./enc-sync.toml` in the current directory, then
  `~/.enc-sync/config.toml` when `--config` is omitted.
- Tilde expansion for `chart_dir` (for example `~/Charts/ENC/US`).
- Example config (`enc-sync.example.toml`) and install notes bundled in release
  archives (`RELEASE-INSTALL.md`).
- Unit tests for catalog parsing, filters, chart update logic, zip extraction, and
  config validation; integration tests with a local HTTP mock server.
- GitHub Actions CI on Ubuntu, macOS, and Windows: `rustfmt`, `clippy`, and tests
  (including Rust 2024 compatibility warnings as errors).
- Multi-platform release pipeline via [cargo-dist](https://github.com/axodotdev/cargo-dist):
  - Tagged releases: static Linux (x86_64 and aarch64 musl), macOS universal
    (Intel + Apple Silicon), and Windows archives on GitHub Releases.
  - Pull request preview builds: after CI lint passes, a Release workflow produces a
    single artifact bundle; the `release/enc-sync` commit status reports build
    success or failure.

### Changed

- Project released under the Apache 2.0 license.

### Fixed

- Validate `chart_dir` at config load (reject empty or file paths); create the chart
  directory at run time when missing.
- Cross-platform home directory resolution for tilde expansion on Windows.
- Portable atomic replacement for catalog and `chartdldr_pi.dat` writes.
- Test helpers that override `HOME` and the current working directory use RAII
  guards and serialization so parallel tests do not flake.
- Malformed required catalog timestamps are reported as errors instead of silently
  skipping cells.
- Release automation: macOS universal `lipo` packaging, PR vs tag artifact cleanup,
  CI gating before publishing PR bundles, structured `dist.toml` target parsing, and
  a maintainable workflow invariant verifier.
