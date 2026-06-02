# enc-sync

Standalone cron-friendly tool that downloads updated US NOAA ENC cells into an
[OpenCPN](https://opencpn.org/) Chart Downloader-compatible directory, then
optionally restarts OpenCPN with a full chart database rebuild (`-D`).

The tool mirrors OpenCPN's Chart Downloader layout: each NOAA or US Army Corps
catalog syncs into its own folder under `{chart_dir}/ENC/` (for example
`ENC/US_CA`, `ENC/US_OR`, `ENC/US_REGION14`, `ENC/US_INLAND`). Within each
folder, enc-sync downloads the catalog, compares cell timestamps to
`chartdldr_pi.dat`, updates changed cells, and extracts zips with the same
`ENC_ROOT/` strip used by the plugin.

NOAA publishes ENC updates **every weekday evening** (Monday–Friday).

## Download and install

### Prebuilt releases

Tagged releases on GitHub include static Linux binaries (x86_64 and aarch64 musl, including
Raspberry Pi), a
macOS universal binary (Intel + Apple Silicon), and a Windows `.exe`. Download the
archive for your platform from the
[Releases](https://github.com/hoffmang9/enc-sync/releases) page.

Archives are named `enc-sync-<version>-<platform>.tar.gz` (or `.zip` on Windows). The
current release line is **1.0.0-rc1** (first release candidate toward 1.0.0). After
extracting, the binary is one directory deep inside the archive (for example
`enc-sync-1.0.0-rc1-x86_64-unknown-linux-musl/enc-sync`). Each archive also includes
this `README.md` for install and usage notes.

Replace `<version>` below with the release you downloaded (for example `1.0.0-rc1`).

### Linux (x86_64 and Raspberry Pi aarch64)

These builds are statically linked with musl.

**x86_64 (typical PC / server):**

```bash
tar xzf enc-sync-<version>-x86_64-unknown-linux-musl.tar.gz
sudo install -m 755 enc-sync-<version>-x86_64-unknown-linux-musl/enc-sync /usr/local/bin/enc-sync
```

**aarch64 (Raspberry Pi 64-bit):**

```bash
tar xzf enc-sync-<version>-aarch64-unknown-linux-musl.tar.gz
sudo install -m 755 enc-sync-<version>-aarch64-unknown-linux-musl/enc-sync /usr/local/bin/enc-sync
```

No glibc version requirements — install to `/usr/local/bin` and run.

### macOS (Intel and Apple Silicon)

The macOS archive contains a universal binary (`x86_64` + `arm64`):

```bash
tar xzf enc-sync-<version>-universal-apple-darwin.tar.gz
install -m 755 enc-sync-<version>-universal-apple-darwin/enc-sync /usr/local/bin/enc-sync
```

#### Gatekeeper (unsigned binary)

macOS Gatekeeper may block an unsigned download on first launch. Use **one** of these once
after extracting:

**Terminal (recommended):**

```bash
xattr -d com.apple.quarantine enc-sync-<version>-universal-apple-darwin/enc-sync
```

**Finder:**

1. Control-click (or right-click) `enc-sync`
2. Choose **Open**
3. Click **Open** in the dialog

After that, run `enc-sync` normally from the terminal.

### Windows

Extract the `.zip` and run `enc-sync.exe` from Command Prompt or PowerShell. The archive
contains a top-level folder with the binary and this README.

#### SmartScreen (unsigned executable)

Windows SmartScreen may warn on first run because the binary is not code-signed. Click
**More info**, then **Run anyway**. This is a one-time prompt for that download.

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
chart_dir = "~/Documents/Charts"

# Sync at least these OpenCPN chart folders (created if missing). enc-sync also
# syncs any other recognized ENC/* folders already on disk.
states = ["CA", "OR", "WA"]      # ENC/US_CA, ENC/US_OR, ENC/US_WA
regions = []                     # e.g. "14" → ENC/US_REGION14
coast_guard_districts = []       # e.g. "13" → ENC/US_CGD13

# Optional minimums:
# all_enc = true                 # at least sync ENC/US (national catalog)
# inland = true                  # at least sync US Army Corps inland folders

restart_opencpn = true
rebuild_chart_db = true
```

Configured states, regions, Coast Guard districts, and optional `all_enc` /
`inland` flags define the **minimum** folders enc-sync always syncs (creating
them if needed). enc-sync **also** syncs any other recognized `ENC/*` folders
already present under `chart_dir` — for example if Chart Downloader added
`ENC/US_FL` but your config only lists CA/OR/WA.

With no config minimums and no recognized folders on disk, enc-sync exits with
an error.

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

While downloading, logs show progress as `Downloading <cell> (N of total)`.

### Catalog-only mode

```bash
enc-sync --config ~/.enc-sync/config.toml --catalog-only
```

Always downloads the latest catalog from NOAA (or ACE) for each selected source,
overwriting any local copy, without downloading chart cells or restarting OpenCPN.

## Cron

Use an explicit path to the machine-wide config:

```cron
0 23 * * 1-5 enc-sync --config ~/.enc-sync/config.toml --cron >>/tmp/enc-sync.log 2>&1
```

With `--cron`, routine per-source progress moves to debug while errors, the source
list summary, and actual cell downloads stay at info. On most days nothing will have
changed and the run exits quietly.

## OpenCPN integration

enc-sync uses the same folder names and catalog URLs as OpenCPN Chart Downloader
(built from OpenCPN's `chart_sources.xml`).

### Recommended setup for CA / OR / WA

1. Set `chart_dir` to your OpenCPN **BaseChartDir** (for example
   `~/Documents/Charts`).
2. Configure `states = ["CA", "OR", "WA"]` in enc-sync.
3. In OpenCPN Chart Downloader, add three catalogs (or let enc-sync create the
   folders on first run):
   - **CA - California** → `ENC/US_CA`
   - **OR - Oregon** → `ENC/US_OR`
   - **WA - Washington** → `ENC/US_WA`
4. Run enc-sync. Each state's catalog and cells live in its own folder.

Chart Downloader's **Update** button works normally per source — each catalog
stays scoped to its folder. No refilter step is needed.

Optional flags:

- `all_enc = true` — at least sync `ENC/US` (national catalog)
- `inland = true` — sync US Army Corps folders `US_INLAND`, `US_INLAND_BUOYS`,
  `US_INLAND_OVERLAYS`

When chart downloads succeed, enc-sync runs `opencpn --remote -q` and relaunches
OpenCPN with `-D` to rebuild the chart database (when configured). If nothing
changed, a running OpenCPN instance is left alone.

## Development

### Build from source

```bash
cargo build --release
```

Install the binary wherever you like:

```bash
cargo install --path .
```

### Pull request preview binaries

PR binaries are **not** on the CI workflow run. After [CI](.github/workflows/ci.yml)
passes, it dispatches the [Release workflow](.github/workflows/release.yml), which builds
and uploads one workflow artifact with the four platform archives.

1. Wait for the PR check **`release/enc-sync`** to turn green (Release can still be running
   after CI is green).
2. Open **Actions** → **Release** (not CI) → the run for your PR commit.
3. Under **Artifacts**, download **`enc-sync-<version>-pr.<number>.<sha>`**. GitHub wraps
   workflow artifact downloads in an extra `.zip`; unzip once to get the four
   `enc-sync-*.tar.gz` / `.zip` files inside.

Internal cargo-dist artifacts are removed when the run finishes — only the bundle above
should remain on PR builds. Tag releases publish to GitHub Releases and leave no workflow
artifacts.

Release builds are not triggered for pull requests from repository forks (GitHub token
scope).

### Cutting a release

Bump the version in `Cargo.toml`, commit, merge to `main`, wait for CI to pass, then tag
that commit and push the tag (`git tag 1.0.0-rc1 && git push origin 1.0.0-rc1`). The tag
name must match the semver in `Cargo.toml` (no `v` prefix). The
[Release workflow](.github/workflows/release.yml) verifies CI succeeded on the tagged
commit, builds all targets, and uploads versioned files to the GitHub Release.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
