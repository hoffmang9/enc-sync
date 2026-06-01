# enc-sync release install notes

Prebuilt binaries for syncing NOAA ENC charts into an OpenCPN Chart Downloader directory.

## Linux (Ubuntu x86_64 and Raspberry Pi aarch64)

These builds are statically linked with musl. Copy the binary anywhere on your `PATH`, for example:

```bash
tar xzf enc-sync-*.tar.gz
sudo install -m 755 enc-sync /usr/local/bin/enc-sync
```

No glibc version requirements — drop it in `/usr/local/bin` and run it.

## macOS (Intel and Apple Silicon)

The macOS archive contains a universal binary (`x86_64` + `arm64`).

```bash
tar xzf enc-sync-*.tar.gz
install -m 755 enc-sync /usr/local/bin/enc-sync
```

### Gatekeeper (unsigned binary)

macOS Gatekeeper will block an unsigned download on first launch. Use **one** of these once after extracting:

**Terminal (recommended):**

```bash
xattr -d com.apple.quarantine ./enc-sync
```

**Finder:**

1. Control-click (or right-click) `enc-sync`
2. Choose **Open**
3. Click **Open** in the dialog

After that, run `enc-sync` normally from the terminal.

## Windows

Extract the `.zip` and run `enc-sync.exe` from Command Prompt or PowerShell.

### SmartScreen (unsigned executable)

Windows SmartScreen may warn on first run because the binary is not code-signed. Click **More info**, then **Run anyway**. This is a one-time prompt for this download.

## Configuration

See the main [README.md](README.md) for config file layout (`enc-sync.toml` or `~/.enc-sync/config.toml`).
