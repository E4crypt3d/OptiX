# Optix

Windows gaming performance optimization and system recovery utility.

> **Optimize your PC without fear. Every change is tracked. Every change can be undone.**

Optix is a system management platform, not a fake "FPS booster". Every
modification follows a fixed pipeline:

**Detect → Snapshot → Apply → Verify → Record → Rollback.**

## Status

- ✅ **Phase 0 — Foundation**: Tauri v2 shell, SQLite schema + migrations,
  error model, elevation bootstrap, CI, scanner + dashboard skeleton.
- 🚧 **Phase 1 — System Scanner**: hardware + software scan with WMI
  enrichment (GPU VRAM, physical disk health/type, motherboard/BIOS, OS build),
  telemetry sampling, health badges. Remaining: GPU & cache panel.
- 🚧 **Phase 2 — Snapshot & Recovery Engine**: snapshot create/list/delete +
  retention, change journal, reverse-order rollback (registry + power domains),
  snapshot diff. Remaining: service/network capture domains + System Restore point.
- 🚧 **Phase 3 — System Cleanup**: safe-category scanner (temp, browser/GPU
  shader caches, crash dumps, logs) with deny-list, snapshot-first deletion,
  policy (keep-newest / age-based). Remaining: bloatware/AppX module,
  SoftwareDistribution + Recycle Bin categories, DISM component cleanup.
- 🚧 **Phase 4 — Process & RAM Management**: process analyzer with
  REQUIRED/SAFE/UNKNOWN classification, kill + priority controls (never
  REALTIME), gaming mode (boost game / lower background / restore on exit).
  Remaining: per-process GPU (PDH), CPU affinity, automatic game-detection
  watcher (Phase 9 integration).
- 🚧 **Phase 5 — Power Management**: power scheme enumeration, Optix profiles
  (Balanced / Competitive / Maximum — cloned plans with processor, PCIe ASPM
  and USB selective-suspend AC values), snapshot-first apply + verify + reverse
  rollback, and NIC power-saving disable (EEE, Green Ethernet, device power
  management) as reversible registry changes. Remaining: processor idle-disable
  preset, per-phase GPU preference (Phase 8).
- 🚧 **Phase 6 — Startup & Service Manager**: service enumeration (state, start
  type, binary path, description, delayed-auto-start) with REQUIRED/SAFE/UNKNOWN
  classification and a hard never-flag list; start/stop and start-type controls;
  Windows Search (WSearch) dedicated toggle; startup app enumeration (Run keys
  + startup folders) with Task Manager `StartupApproved` disabled-state
  awareness and reversible enable/disable. Remaining: scheduled-task
  enumeration, publisher/signature verification.
- ⬜ Phase 7–12 — Network, GPU, Game Profiles, Benchmark, Crash Recovery,
  Diagnostics

## Stack

- **Frontend**: React 19 + TypeScript, Tailwind CSS v4, Vite, Recharts
- **Backend**: Rust, Tauri v2, `sysinfo` (cross-platform scanner), `rusqlite`
  (bundled SQLite), `windows-sys` + `winreg` (Windows integration)
- **Database**: SQLite at `C:\ProgramData\Optix\optix.db`

## Architecture

```
src/                     React frontend (Vite + Tailwind)
src-tauri/
  src/
    main.rs              elevation bootstrap + entry point
    lib.rs               module tree + command registration
    error.rs             OptixError (thiserror, serialized to frontend)
    commands/            Tauri commands (system.rs, processes.rs, …)
    db/                  SQLite schema + migrations (sqlite.rs)
    engine/              cleanup / snapshot / rollback / optimizer / power / processes / services
    models/              hardware / snapshot / optimization / power / process / services structs
    win/                 #[cfg(windows)]: elevation, registry, GDI, power, nic, services, startup, process
```

## Development

```bash
npm install
npm run tauri dev          # runs a Linux build on Linux hosts
```

Windows-only modules are gated behind `#[cfg(windows)]` and Windows-only crates
are declared under `[target.'cfg(windows)'.dependencies]`, so the project still
compiles and runs on Linux for UI/engine development. Real Windows integration
is verified in CI (`windows-latest`).

## Building the Windows installer

From a Windows host (NSIS + MSI):

```bash
npm run tauri build
```

From Linux (NSIS only — WiX/MSI runs only on Windows):

```bash
rustup target add x86_64-pc-windows-msvc
cargo install --locked cargo-xwin
sudo apt install lld llvm clang nsis   # clang is needed for rusqlite's C build
npm run tauri build -- --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis
```

## Security model

- Every mutation is snapshot-first and reversible via the rollback engine.
- v1 runs as a single elevated process (the app relaunches itself through UAC
  when not already elevated). A privileged-service split is the production plan.
- Never: irreversible changes, auto-delete, permanent security-feature
  changes, or REALTIME process priority.

## CI

GitHub Actions runs backend tests on Linux and builds NSIS + MSI installers on
`windows-latest`.
