# enc-sync

Standalone cron-friendly tool that downloads updated US NOAA ENC cells into an
[OpenCPN](https://opencpn.org/) Chart Downloader-compatible directory, then
optionally restarts OpenCPN with a full chart database rebuild (`-D`).

The tool mirrors OpenCPN's Chart Downloader behavior: it reads NOAA's
`ENCProdCat.xml`, compares cell timestamps to `chartdldr_pi.dat` and on-disk
cells, downloads only what changed, and extracts zips with the same `ENC_ROOT/`
strip used by the plugin.

NOAA publishes ENC updates **every weekday evening** (Monday–Friday).

## Build

From source:

```bash
cargo build --release
```

Install the binary wherever you like, for example:

```bash
cargo install --path .
```

### Prebuilt releases

Tagged releases on GitHub include static Linux binaries (x86_64 and aarch64 musl), a
macOS universal binary (Intel + Apple Silicon), and a Windows `.exe`. Download the
archive for your platform from the [Releases](https://github.com/hoffmang9/enc-sync/releases)
page.

Release archives are versioned in the filename, for example
`enc-sync-0.1.0-x86_64-unknown-linux-musl.tar.gz`. On pull requests, the [Release
workflow](.github/workflows/release.yml) runs only after [CI](.github/workflows/ci.yml)
passes and publishes one workflow artifact named `enc-sync-<version>-pr.<number>.<sha>`
containing only the four platform archives (Linux x86_64, Linux aarch64/Pi, macOS universal,
Windows). A separate PR check **`release/enc-sync`** reports Release success or failure on
the commit (CI can pass while Release is still running or if it fails). Other artifacts on
the Release run (manifest JSON, per-job build zips) are CI internals — ignore those when
testing a PR build. Release builds are not triggered for pull requests from repository
forks (GitHub token scope).

Each archive includes `RELEASE-INSTALL.md` with platform-specific install steps and
notes on macOS Gatekeeper and Windows SmartScreen for unsigned binaries.

To cut a release, bump the version in `Cargo.toml`, commit, merge to `main`, wait for CI
to pass, then tag that commit and push the tag (`git tag v0.1.0 && git push origin v0.1.0`).
The [Release workflow](.github/workflows/release.yml) verifies CI succeeded on the tagged
commit, builds all targets, and uploads versioned files to the GitHub Release.

## Configure

Copy the example config and edit it. For day-to-day use in a checkout or project
directory, keep a local config:

```bash
cp enc-sync.example.toml enc-sync.toml
```

For a machine-wide default (typical for cron), install under your home directory:

```bash
mkdir -p ~/.enc-sync
cp enc-sync.example.toml ~/.enc-sync/config.toml
```

```toml
chart_dir = "~/Charts/ENC/US"
catalog_url = "https://www.charts.noaa.gov/ENCs/ENCProdCat.xml"

# A cell is updated if it matches ANY non-empty list below.
states = ["CA", "OR", "WA"]
regions = []                 # e.g. "14", "15"
coast_guard_districts = []     # e.g. "11", "13"

restart_opencpn = true
rebuild_chart_db = true
```

Leave all three filter lists empty to keep the entire catalog current.

## Run

From a directory that contains `enc-sync.toml`:

```bash
enc-sync
```

Or pass an explicit path:

```bash
enc-sync --config ~/.enc-sync/config.toml
```

If `--config` is omitted, `enc-sync` looks for config in this order:

1. `./enc-sync.toml` in the current directory
2. `~/.enc-sync/config.toml`

## Cron

Use an explicit path to the machine-wide config:

```cron
0 23 * * 1-5 enc-sync --config ~/.enc-sync/config.toml >>/tmp/enc-sync.log 2>&1
```

On most days nothing will have changed and the run exits quickly.

## OpenCPN integration

- Chart directory layout and `chartdldr_pi.dat` are compatible with OpenCPN's
  Chart Downloader plugin.
- When downloads succeed, the tool runs `opencpn --remote -q` and relaunches
  OpenCPN with `-D` to rebuild the chart database.
- If nothing changed, a running OpenCPN instance is left alone.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
