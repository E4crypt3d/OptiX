# Bundled resources

`PresentMon64.exe` (from Intel's PresentMon, MIT) is downloaded into this
directory by `scripts/fetch-presentmon.ps1` and bundled by the Windows release
build (see `bundle.resources` in `../tauri.conf.json`).

The file is intentionally not committed (it is ~1 MB and fetched from GitHub
releases). This README keeps the directory non-empty so dev builds that glob
`resources/*` succeed on a fresh checkout.
