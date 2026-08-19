# Changelog

All notable changes to Optix will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
