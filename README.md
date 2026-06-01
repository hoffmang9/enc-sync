# enc-sync

Standalone cron-friendly tool that downloads updated US NOAA ENC cells into an
[OpenCPN](https://opencpn.org/) Chart Downloader-compatible directory, then
optionally restarts OpenCPN with a full chart database rebuild (`-D`).

The tool mirrors OpenCPN's Chart Downloader behavior: it reads NOAA's
`ENCProdCat.xml`, compares cell timestamps to `chartdldr_pi.dat` and on-disk
cells, downloads only what changed, and extracts zips with the same `ENC_ROOT/`
strip used by the plugin.

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

While downloading, logs show progress as `Downloading <cell> (N of total)`.

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
