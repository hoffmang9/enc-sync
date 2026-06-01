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

```bash
cargo build --release
```

Install the binary wherever you like, for example:

```bash
cargo install --path .
```

## Configure

Copy the example config and edit it:

```bash
mkdir -p ~/.config/enc-sync
cp enc-sync.example.toml ~/.config/enc-sync/config.toml
```

```toml
chart_dir = "/path/to/Charts/ENC/US"
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

```bash
enc-sync --config ~/.config/enc-sync/config.toml
```

If `--config` is omitted, `enc-sync` looks for `enc-sync.toml` in the current
directory, then `~/.config/enc-sync/config.toml`.

## Cron

```cron
0 23 * * 1-5 enc-sync --config ~/.config/enc-sync/config.toml >>/tmp/enc-sync.log 2>&1
```

On most days nothing will have changed and the run exits quickly.

## OpenCPN integration

- Chart directory layout and `chartdldr_pi.dat` are compatible with OpenCPN's
  Chart Downloader plugin.
- When downloads succeed, the tool runs `opencpn --remote -q` and relaunches
  OpenCPN with `-D` to rebuild the chart database.
- If nothing changed, a running OpenCPN instance is left alone.

## License

GPL-2.0-or-later
