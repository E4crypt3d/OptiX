# Changelog

All notable changes to Optix will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **System Restore points**: `SRSetRestorePoint` command on the Snapshots page
  and an automatic best-effort restore point before cleanup / DISM runs.
- **Cleanup expansion**: SoftwareDistribution + Delivery Optimization cache
  category (skipped while `wuauserv` is running), Recycle Bin category via
  `SHQueryRecycleBin`/`SHEmptyRecycleBin`, and DISM WinSxS
  `/startcomponentcleanup` with streamed output.
- **Per-process GPU usage**: PDH `\GPU Engine` counters summed per PID, shown
  as a GPU column in Processes (Task-Manager-style).
- **ICMP ping test**: `IcmpSendEcho` on Windows (`ping` subprocess on Linux)
  with median/min/max RTT, jitter, loss, and the full sample series.
- **TCP/IP tweaks**: apply recommended values and one-click revert to driver
  defaults, snapshot-first with per-write verification.
- **Scheduled tasks**: `schtasks /fo CSV /v` enumeration with Authenticode
  signature verification of each action executable.
- **NVIDIA DRS per-game profiles**: runtime-loaded NVAPI (no SDK dependency),
  byte-exact SDK struct layouts; applies power-mode + shader-cache settings to
  an `Optix: <game>` profile bound to the game executable, removable as the
  rollback path.
- **PresentMon bundling**: `scripts/fetch-presentmon.ps1` + `bundle.resources`
  so release builds ship the benchmark capture binary; CI fetches it before
  packaging.
- **Live crash-watch**: background thread polls the Application event log and
  emits `optix://crash-detected`; the Crash Reports page refreshes live.
- **Backend logging**: every error is written to the console and `logs.txt`
  next to the installed executable (falling back to the data dir); panic hook
  captures backend panics; `log_path` exposed in app info.

### Fixed

- Legacy-snapshot migration test (rusqlite 0.40 `query_row` closure contract).

## [0.5.0] - 2026-08-19

### Added

- **Linux support**: builds as `.deb` and AppImage from CI; Linux install
  instructions added to README.
- **LTTB decimation**: Largest-Triangle-Three-Buckets algorithm for benchmark
  frame-time charts — caps chart data at 1500 points while preserving visual
  spikes.
- **Visibility-aware polling**: `useInterval` hook pauses telemetry polling
  when the window is hidden, so the dashboard idles at zero cost when
  minimized.
- **Lazy-loaded views**: all 16 views loaded on demand via `React.lazy()` +
  `Suspense` — heavy dependencies (recharts, etc.) are only parsed when the
  user opens that page.
- **content-visibility CSS**: browser rendering optimization for off-screen
  cards and long scrollable lists.

### Fixed

- Race condition in Rollback.tsx snapshot-change loading.

## [0.4.0] - 2026-08-19

### Added

- **File browser for game executable selection**: Browse button in the manual
  game entry form opens a native file picker filtered to .exe/.cmd/.bat files,
  using `tauri-plugin-dialog` for cross-platform file selection.

## [0.3.0] - 2026-08-19

### Added

- **Settings page**: application info (version, data directory, snapshots
  directory, snapshot retention) and safety model documentation explaining
  the Detect → Snapshot → Apply → Verify → Record → Rollback pipeline.

## [0.2.0] - 2026-08-19

### Added

- **Phase 6 — Startup & Service Manager**: service enumeration with
  REQUIRED/SAFE/UNKNOWN classification, start/stop and start-type controls,
  Windows Search toggle, startup app enumeration with Task Manager
  `StartupApproved` awareness and reversible enable/disable.
- **Phase 7 — Network Optimization**: cross-platform UDP DNS benchmark
  (median/p95/min latency + packet loss), network status, snapshot-first DNS
  apply with cache flush, and read-only TCP/IP parameters.
- **Phase 8 — GPU Management**: display-adapter summary, risk-tiered gaming
  toggles (HAGS, Game DVR, Game Bar, Memory Integrity, Game Mode, MPO),
  shader-cache inventory + safe clear (NVIDIA/AMD/DirectX), and AMD
  shader-cache mode read/write.
- **Phase 9 — Game Profile System**: launcher detection (Steam, Epic,
  Riot/Battle.net) with VDF parser, saved game library, per-game profiles
  (CPU priority, affinity, power/network/gpu), and background game-mode
  watcher with auto-apply on launch.
- **Phase 10 — Benchmark System**: PresentMon CSV parsing with frame-time
  percentile math, config-hash for before/after grouping, benchmark runner,
  system-stress fallback, history + comparison, and frame-time chart.
- **Phase 11 — Crash Recovery**: Application Event Log scan (1000/1001/4101),
  WER `Report.wer` parsing, minidump discovery, exception-code classification,
  dedup across sources, and `CrashReport.zip` generation.
- **Phase 12 — AI Diagnostics**: rule-based diagnostic engine with 11 rules
  (memory pressure, disk fullness, CPU/GPU bottlenecks, driver crashes/TDR,
  thermal, background apps) with confidence scoring and ranked findings.
- **Phase 13 — Bloatware/AppX Removal**: AppX package scanner with
  classification (32 protected, 27 removal, 2 caution), snapshot-first
  removal, provisioned-copy cleanup, and rollback support via
  `Add-AppxPackage -Register`.

### Fixed

- Extracted `errMsg()` utility for readable error display across all frontend
  components (Tauri `invoke` errors now show human-readable messages instead
  of `[object Object]`).
- Made database migration idempotent with `ensure_column()` backfill for
  columns added after a table's first release.

## [0.1.0] - 2026-08-15

### Added

- **Phase 0 — Foundation**: Tauri v2 shell, SQLite schema + migrations,
  error model, elevation bootstrap, CI, scanner + dashboard skeleton.
- **Phase 1 — System Scanner**: hardware + software scan with WMI enrichment
  (GPU VRAM, physical disk health/type, motherboard/BIOS, OS build),
  telemetry sampling, health badges.
- **Phase 2 — Snapshot & Recovery Engine**: snapshot create/list/delete +
  retention, change journal, reverse-order rollback (registry + power
  domains), snapshot diff.
- **Phase 3 — System Cleanup**: safe-category scanner (temp, browser/GPU
  shader caches, crash dumps, logs) with deny-list, snapshot-first deletion,
  policy (keep-newest / age-based).
- **Phase 4 — Process & RAM Management**: process analyzer with
  REQUIRED/SAFE/UNKNOWN classification, kill + priority controls (never
  REALTIME), gaming mode, and CPU-affinity control.
- **Phase 5 — Power Management**: power scheme enumeration, Optix profiles
  (Balanced / Competitive / Maximum), snapshot-first apply + verify +
  reverse rollback, and NIC power-saving disable.
