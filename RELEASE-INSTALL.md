# enc-sync release install notes

Prebuilt binaries for syncing NOAA ENC charts into an OpenCPN Chart Downloader directory.

Archives are named `enc-sync-<version>-<platform>.tar.gz` (or `.zip` on Windows). After
extracting, the binary lives one directory deep inside the archive.

## Linux (Ubuntu x86_64 and Raspberry Pi aarch64)

These builds are statically linked with musl. Example for x86_64:

```bash
tar xzf enc-sync-0.1.0-x86_64-unknown-linux-musl.tar.gz
sudo install -m 755 enc-sync-x86_64-unknown-linux-musl/enc-sync /usr/local/bin/enc-sync
```

For Raspberry Pi 64-bit, use the `aarch64-unknown-linux-musl` archive instead:

```bash
tar xzf enc-sync-0.1.0-aarch64-unknown-linux-musl.tar.gz
sudo install -m 755 enc-sync-aarch64-unknown-linux-musl/enc-sync /usr/local/bin/enc-sync
```

No glibc version requirements — drop it in `/usr/local/bin` and run it.

## macOS (Intel and Apple Silicon)

The macOS archive contains a universal binary (`x86_64` + `arm64`):

```bash
tar xzf enc-sync-0.1.0-universal-apple-darwin.tar.gz
install -m 755 enc-sync-universal-apple-darwin/enc-sync /usr/local/bin/enc-sync
```

### Gatekeeper (unsigned binary)

macOS Gatekeeper will block an unsigned download on first launch. Use **one** of these once after extracting:

**Terminal (recommended):**

```bash
xattr -d com.apple.quarantine enc-sync-universal-apple-darwin/enc-sync
```

**Finder:**

1. Control-click (or right-click) `enc-sync`
2. Choose **Open**
3. Click **Open** in the dialog

After that, run `enc-sync` normally from the terminal.

## Windows

Extract the `.zip` and run `enc-sync.exe` from Command Prompt or PowerShell. The archive
contains a top-level directory with the binary and install notes.

### SmartScreen (unsigned executable)

Windows SmartScreen may warn on first run because the binary is not code-signed. Click **More info**, then **Run anyway**. This is a one-time prompt for this download.

## Configuration

See the main [README.md](README.md) for config file layout (`enc-sync.toml` or `~/.enc-sync/config.toml`).
