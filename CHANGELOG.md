# Changelog

All notable changes to Optix will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.18.0] - 2026-08-20

### Added

- **Per-view error boundary**: a page that crashes while rendering now shows
  the error with Retry / Go to Dashboard buttons instead of blanking the
  whole app; switching tabs resets the boundary.
- **Frontend errors now reach `logs.txt`**: uncaught errors, unhandled
  promise rejections, and render crashes are forwarded to the backend log via
  a new `log_event` command (level-whitelisted, length-capped), so frontend
  failures are visible in release builds too.
- **Global keyboard-focus outline** and `prefers-reduced-motion` support; the
  sidebar marks the active page with `aria-current`.
- **Telemetry retention**: `hardware_history` is pruned to the newest 2,000
  samples, so the dashboard's 30-second recorder can no longer grow the
  database without bound.

### Changed

- **Smooth dashboard updates**: sampling raised from 1.5 s to 1 s, and CPU /
  memory / network values now glide between samples at 60 fps via
  `requestAnimationFrame` interpolation, with re-renders isolated to the
  affected numbers (the chart and cards re-render only once per second). The
  history chart window is relabeled to the last 60 seconds, and download /
  upload show an unavailable state until the first sample arrives.
- **SQLite `busy_timeout` (5 s)**: the game watcher's second WAL connection
  now waits out transient writer contention instead of failing with
  `SQLITE_BUSY`.
- **Dashboard error banner** renders readable backend messages instead of
  `[object Object]`.

### Fixed

- **Snapshot ids are validated** before any filesystem join (delete / restore
  / diff), closing a path-traversal path for malformed ids.
- **Linux ping passes `--` before the host** so a hostname beginning with `-`
  cannot be parsed as a command option.

## [0.17.0] - 2026-08-20

### Added

- **Memory state card** on the Processes & RAM page: total/used/available RAM,
  cached (Linux) / committed (Windows), swap, and a pressure level
  (normal / elevated / critical).
- **Suspend / resume processes** on both platforms: Windows resolves
  `NtSuspendProcess`/`NtResumeProcess` from ntdll at runtime (the mechanism
  Process Explorer uses); Linux uses `SIGSTOP`/`SIGCONT`.
- **CPU-affinity core picker** (Windows): read a process's affinity mask plus
  the system mask and pin it to a chosen set of cores.
- **Detect active game**: copy the PID owning the foreground window into the
  gaming-mode game list (Windows `GetForegroundWindow`; Linux `xdotool`).
- **Process table sorting** (name / CPU / RAM / disk / threads) and status
  badges (suspended / zombie / dead / idle).
- **Thread count and owning user id** per process (uid on Linux; null on
  Windows where sysinfo exposes a SID).

### Changed

- **Shared process-sampling state**: one `System` instance is reused across
  refreshes, so CPU percentages are measured over the real elapsed interval
  (no fixed sleep on every refresh) and memory is never re-read twice.
- **Live monitoring**: the process list auto-refreshes every 5 s while the
  page is visible, pausing entirely when the window is hidden.
- **`get_affinity` returns the system mask** along with the per-process mask.
- **Gaming mode**: added "Detect active game" button; a process must be
  running (not dead/zombie) to be controlled.
- **Cross-platform build scripts** added to the package manifest
  (`build:win`, `build:nsis`, `build:linux`) and the Windows
  `Win32_System_SystemInformation` windows-sys feature enabled for the commit
  charge read.

## [0.16.0] - 2026-08-20

### Added

- **Power profile preview**: the Power page now shows exactly what applying a
  profile would change (per-setting current → Optix target on the active
  scheme) before you confirm, so you can review the deltas first.
- **Current power state card**: shows the active scheme name, AC/battery
  status, and the current vs recommended value of every tracked setting
  (processor min/max, PCIe ASPM, USB selective suspend). A battery note warns
  that AC-only settings are applied.
- **Idempotent apply fast path**: applying a profile whose active scheme
  already matches it (name + all tracked settings at target) is a no-op — no
  clone, writes, or snapshot.
- **Accurate GPU VRAM**: NVIDIA `HardwareInformation.qwMemorySize` (bytes) /
  AMD `MemorySize` (MB) from the display driver registry, replacing WMI's
  32-bit `AdapterRAM` which capped at 4 GiB; Linux VRAM from sysfs
  `mem_info_vram_total` and NVIDIA `/proc` information.
- **Linux hardware detection**: physical disks from sysfs (`/sys/block`);
  motherboard/BIOS from DMI (`/sys/class/dmi/id`); startup apps from
  freedesktop autostart entries; display refresh rate from the preferred mode
  line.
- **Scanner loading skeleton** and new CPU Usage / Memory Usage / Network
  cards; Motherboard & BIOS card now always shown (dashes when unreadable).

### Changed

- **Motherboard/BIOS/edition read in one WMI connection** (`SystemHardware`)
  instead of three separate connections, reducing scan latency; edition is
  captured during the scan rather than a separate enrich pass.
- **Scanner populates physical disks** (previously always empty) on both
  platforms.
- **`PowerDeleteScheme` treats an already-missing scheme as success** so
  rollback re-runs after a re-apply don't fail.
- **Renamed power settings to tracked settings** with labels, enforced to be
  exactly the four that profiles write (validated by a unit test).

## [0.15.0] - 2026-08-20

### Added

- **Legacy DB migrations**: first-release `changes`, `hardware_history`, and
  `benchmarks` tables (shipped with NOT NULL legacy columns) are rebuilt to
  the current schema on startup, mapping legacy columns over so existing
  records survive.
- **Restore progress in Rollback page**: the restore button shows a
  "Restoring…" state and a confirmation prompt, the snapshot list gets a
  manual Refresh action and created/restored timestamps, and changes load
  with a loading state.

### Changed

- **DB is the single source of truth for changes**: `record_change` no longer
  appends to an on-disk `changes.json` (removed at snapshot creation too),
  eliminating a side file to keep in sync or race on.
- **Async restore**: `restore_snapshot` applies the per-change rollbacks
  (registry writes, service starts, appx reinstalls) off the main thread.
- **Snapshot marked restored only on full success**: a snapshot stays active
  on partial failure so the user can retry, and is stamped with a restore
  timestamp only when every change succeeded.
- **Serialized restores**: a global lock prevents two concurrent restores of
  the same snapshot from double-applying non-idempotent domains.
- **Rollback continues past individual failures**: a single bad change no
  longer aborts the remaining reversions; the error aggregates reverted/failed
  counts.
- **Non-reversible changes are skipped, not failed**: recorded file deletions
  are counted as neither reverted nor failed.
- **Diff noise removed**: snapshot diffs skip `changes` / `timestamp`
  bookkeeping files, leaving only captured state.

## [0.14.0] - 2026-08-20

### Added

- **Log file in Settings**: the app-info block now shows the log file path,
  noting that errors and warnings are written there so nothing fails silently.

### Fixed

- **DISM output deadlock**: DISM component cleanup drains stdout and stderr
  on separate threads, so a child that fills its stderr pipe can no longer
  block the operation.
- **Benchmark capture cleanup**: PresentMon is resolved before sampling
  starts, and a failed capture removes the partial CSV instead of leaving it
  behind.
- **Benchmark delete cleanup**: `delete_benchmark` also removes the run's
  capture CSV (best-effort; a missing file is tolerated).
- **PresentMon PATH lookup**: `find_presentmon` explicitly checks the
  executable on `PATH`, so the UI's "or on PATH" guidance is accurate.
- **PresentMon window flash**: `run_capture` uses `CREATE_NO_WINDOW` so a
  capture doesn't flash a console for its duration.
- **Crash-watch false new-alert burst**: the watcher's first poll only primes
  the watermark, so crashes that pre-date app launch are never reported as
  new.
- **Event-log handle leak**: batched event handles are closed before
  returning.
- **Minidump memory spike**: crash-report zips stream minidumps via
  `io::copy` instead of buffering up to 200 MB.
- **Bottleneck rankings**: CPU/GPU bottleneck diagnostics analyze the newest
  run (newest-first list), not the oldest.
- **Disk-free diagnostic**: uses the worst fixed disk (removable media
  excluded) instead of a cross-disk aggregate that could hide a nearly-full
  system drive.

### Changed

- **Platform-aware Benchmark page**: FPS capture (game / process picker,
  PresentMon) is hidden on non-Windows; the stress test remains
  cross-platform.
- **Platform-aware Crash Reports page**: read-only banner on non-Windows and
  the scan control hidden.
- **Linux cleanup safety and speed**: the current user's UID is read once per
  directory walk and only current-user-owned files are candidates.
- **Async frame-times**: `benchmark_frame_times` reads and parses the CSV off
  the main thread.
- **Cheap service query**: the Windows Update guard now uses a single
  `service_running("wuauserv")` SCM query instead of a full service
  enumeration.

## [0.13.0] - 2026-08-20

### Added

- **NVIDIA DRS per-game profiles**: applying a game profile with
  `gpu_profile: nvidia` now creates a driver-settings (`nvapi64.dll` DRS)
  profile with a per-game power/shader-cache preference, surfaced as an
  "NVIDIA driver profile" toggle on the Games page (Windows only).
- **DRS profile rollback**: the created NVIDIA profile is recorded in the
  apply snapshot under a new `nvapi_profile` `gpu`-domain rollback kind, so
  Rollback Center can remove it again.
- **DRS profile cleanup on remove**: deleting a game that had an NVIDIA
  profile removes its driver-settings profile (best-effort; failures are
  logged rather than blocking the delete).
- **GPU profile validation**: `validate_profile` now rejects unknown
  `gpu_profile` values (only `nvidia` is supported).

### Changed

- **Reused process snapshot in game watcher**: the game-mode watcher keeps a
  single `System` alive across its 2 s polls (`process_names(sys)`) instead of
  re-allocating the process table every pass.
- **Watcher deadlock fix**: per-game state is loaded before touching the
  active-process map so the database is never read while holding the lock.
- **Async game commands**: `detect_games` and `list_games` now run file,
  registry, and process enumeration off the main thread, keeping the UI
  responsive while the Games page polls.

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
