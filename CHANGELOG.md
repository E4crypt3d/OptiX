# Changelog

All notable changes to Optix will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.12.0] - 2026-08-20

### Added

- **Reversible AMD shader cache**: `set_amd_shader_cache` is now snapshot-first,
  writes the `UMD\ShaderCache` value, and verifies the write landed. On
  failure the previous bytes are restored and an error is returned.
- **GPU rollback domain**: raw REG_BINARY changes (which the generic registry
  rollback cannot restore) are recorded under the `gpu` domain and reverted by
  `win::gpu::rollback_gpu`.
- **AMD shader cache confirmation**: the GPU page asks for confirmation before
  changing the cache mode, noting that a snapshot is created first and the
  change is reversible.

### Changed

- **Async adapter enumeration**: `list_gpu_adapters` now runs WMI-backed
  detection off the main thread via `spawn_blocking`, keeping the UI
  responsive.
- **Platform-aware GPU page**: gaming toggles, shader-cache clearing, and the
  selected-bytes counter are hidden on non-Windows platforms with a read-only
  explanation; driver version is only shown when present.

## [0.11.0] - 2026-08-20

### Added

- **Parallel DNS benchmark**: resolvers are now probed on separate threads so
  a slow or unreachable server does not stall the entire benchmark run.
- **DNS apply validation**: rejects unknown adapter GUIDs and malformed server
  lists (must be 1-4 valid IPv4 addresses) at the backend boundary before
  any registry write.
- **TCP tweak revert verification**: `reset_tcp_tweaks` now checks that the
  registry deletion landed; on failure the previous value is restored and an
  error is returned.
- **Ping jitter improvement**: jitter is now computed as median absolute
  deviation between consecutive RTTs, replacing the mean which is sensitive
  to outliers.

### Fixed

- **DNS flush window flash**: `ipconfig /flushdns` now runs with
  `CREATE_NO_WINDOW` to prevent a console window from briefly appearing in
  the GUI application.

### Removed

- **Dead network commands**: removed `list_dns_servers` and
  `tcp_parameters` commands, `TcpParameter` model, and their frontend
  bindings — these were superseded by `benchmark_dns` and `list_tcp_tweaks`.

### Changed

- **Platform-aware Network page**: TCP/IP tweak action buttons are hidden on
  non-Windows platforms; a read-only banner is shown instead.
- **Platform-aware StartupServices page**: Windows Search, Scheduled Tasks,
  and Services sections display platform-aware empty states instead of
  generic "no entries" messages.

## [0.10.0] - 2026-08-20

### Added

- **Power-profile abort safety**: cloned scheme is deleted if audit recording
  fails, preventing orphaned power plans.
- **NIC power-saving early exit**: returns immediately with no snapshot when
  no adapter has power-saving enabled, avoiding unnecessary operations.
- **NIC per-write rollback**: each registry write rolls back on audit failure,
  preventing partial state when the journal cannot record a change.
- **Optional NIC snapshot**: `snapshotId` is now nullable; no snapshot is
  created when there is nothing to change.
- **Power page platform awareness**: read-only banner on non-Windows, loading
  skeletons, scheme availability badges, and disabled states when the base
  scheme is missing.

## [0.9.0] - 2026-08-20

### Added

- **Linux GPU detection**: `lspci`-based adapter enumeration with automatic
  vendor inference (NVIDIA / AMD / Intel), sysfs fallback for systems without
  pciutils.
- **Linux display detection**: `/sys/class/drm` connector enumeration with
  internal-panel (eDP/LVDS) prioritization for primary-display heuristics.
- **Linux cleanup paths**: browser-cache profiles (Chrome, Edge, Chromium,
  Firefox), shader-cache directories (Mesa, NVIDIA, AMD), and user-owned
  temp-directory cleanup — `/tmp` is never touched.
- **Cleanup input validation**: `validate_ids()` rejects unknown or empty
  category IDs at the backend boundary before snapshot or filesystem
  operations.
- **Cleanup selection helpers**: "Select safe", "Select all", "Clear" buttons
  with live total-bytes indicator.
- **Bloatware selection helpers**: "Select candidates", "Select all removable",
  "Select visible", "Clear" with per-package count.
- **Platform-aware pages**: Cleanup and Bloatware pages detect non-Windows
  platforms and show read-only banners; DISM section is hidden on Linux.
- **Improved confirmation dialogs**: caution-package and provisioned-package
  warnings in Bloatware; rebuild-warning for GPU shader caches in Cleanup.
- **Improved result banners**: structured success sections with per-category
  breakdown, snapshot badge, and individual failure details.
- **Loading skeletons**: animated pulse placeholders on Cleanup and Bloatware
  while the initial scan runs.

### Fixed

- **PowerShell injection**: single-quote escaping in `remove_installed` and
  `remove_provisioned` prevents package names with embedded quotes from
  breaking the command.
- **Cleanup size overflow**: saturating arithmetic on file-size sums prevents
  panics on extremely large directories.
- **File ownership check**: cleanup skips files not owned by the current user
  on Linux, preventing deletion of other users' temp files.
- **Empty-category filtering**: cleanup scan now drops categories with zero
  files/bytes from the results list.
- **Temperature label truncation**: Scanner temperature labels now truncate
  gracefully with a tooltip for long sensor names.

## [0.8.0] - 2026-08-20

### Added

- **Multi-display support**: enumerate all active Windows displays with name,
  resolution, refresh rate, and primary-display indicator; falls back to
  `EnumDisplaySettingsW`, `GetSystemMetrics`, and a "Primary display" sentinel
  for unusual drivers.
- **GPU detection overhaul**: three-source merge (registry, WMI, EnumDisplayDevices)
  with name-based matching instead of index-based pairing — integrated GPUs on
  dual-adapter laptops are no longer hidden.
- **Dashboard rescan button**: manual hardware re-scan from the Dashboard header
  with loading spinner and error recovery.
- **Redesigned per-core usage cards**: card-per-core layout with percentage
  badge, responsive grid up to 6 columns.
- **Dedicated Graphics adapters and Displays cards**: GPU card shows vendor,
  driver version, and VRAM; display card shows name, primary badge, and
  resolution/refresh-rate per monitor.
- **CPU vendor** shown on the Dashboard CPU card.
- **Progress bar accessibility**: `role="progressbar"` with `aria-valuemin`,
  `aria-valuemax`, and `aria-valuenow`.
- **Sub-minute uptime formatting**: `formatUptime` now renders seconds when
  under one minute.

### Fixed

- GPU VRAM enrichment now uses name-based WMI matching instead of index
  pairing, preventing wrong-card VRAM on multi-GPU systems.

## [0.7.0] - 2026-08-20

### Added

- **Accurate network throughput**: per-window receive/transmit rates
  (bytes/sec) computed in the backend from the shared refresh delta, shown on
  the Dashboard.
- **Persistent Dashboard state**: chart history and hardware scan survive tab
  switches (module-level cache with 60s TTL), and the chart backfills from
  persisted samples on first visit.
- **Scan error banner** with retry, and live uptime that counts up between
  scans.

## [0.6.0] - 2026-08-20

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
