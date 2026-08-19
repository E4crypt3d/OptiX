# Optix — Research-Backed Development Plan

**Project:** Optix — Windows Gaming Performance Optimization + System Recovery Utility
**Target platforms:** Windows 11 (primary), Windows 10 (secondary, EOL)
**Dev environment:** Linux host (cross-compile to Windows)
**Last updated:** 2026-08-19
**Status:** Research complete — validated APIs, crates, registry keys, and caveats for every phase

> Core promise: **"Optimize your PC without fear. Every change is tracked. Every change can be undone."**
> Every modification follows: Detect → Snapshot → Apply → Verify → Record → Rollback.

---

## Table of Contents

1. [Research Summary & Key Decisions](#1-research-summary--key-decisions)
2. [Platform Reality Check](#2-platform-reality-check)
3. [Technology Stack (Validated Versions)](#3-technology-stack-validated-versions)
4. [Development Environment: Linux → Windows](#4-development-environment-linux--windows)
5. [Architecture & Privilege Model](#5-architecture--privilege-model)
6. [Database Schema](#6-database-schema)
7. [Dashboard Specification (incl. GPU & Cache Panel)](#7-dashboard-specification)
8. [Phase 0 — Foundation](#8-phase-0--foundation)
9. [Phase 1 — System Scanner](#9-phase-1--system-scanner)
10. [Phase 2 — Snapshot & Recovery Engine](#10-phase-2--snapshot--recovery-engine)
11. [Phase 3 — System Cleanup](#11-phase-3--system-cleanup)
12. [Phase 4 — Process & RAM Management](#12-phase-4--process--ram-management)
13. [Phase 5 — Power Management](#13-phase-5--power-management)
14. [Phase 6 — Startup & Service Manager](#14-phase-6--startup--service-manager)
15. [Phase 7 — Network Optimization](#15-phase-7--network-optimization)
16. [Phase 8 — GPU Management](#16-phase-8--gpu-management)
17. [Phase 9 — Game Profile System](#17-phase-9--game-profile-system)
18. [Phase 10 — Benchmark System](#18-phase-10--benchmark-system)
19. [Phase 11 — Crash Recovery](#19-phase-11--crash-recovery)
20. [Phase 12 — AI Diagnostics (Optional)](#20-phase-12--ai-diagnostics-optional)
21. [Windows 10 vs Windows 11 Matrix](#21-windows-10-vs-windows-11-matrix)
22. [Security Rules](#22-security-rules)
23. [Risk Register & Honest Expectations](#23-risk-register--honest-expectations)
24. [Testing & QA Strategy](#24-testing--qa-strategy)
25. [Milestones](#25-milestones)
26. [Source References](#26-source-references)

---

## 1. Research Summary & Key Decisions

| # | Decision | Rationale (research-backed) |
|---|----------|------------------------------|
| 1 | **Win11 first, Win10 second.** Both share the same Win32 API surface (all required APIs are Vista+). Win10 support ended **Oct 14, 2025** (only paid ESU remains) — but ~30-40% of Steam users still run it, so keep it working. | MS end-of-support page; Steam HW survey 2026 |
| 2 | **All Windows-specific Rust code behind `#[cfg(target_os = "windows")]`.** The app must run in dev mode on Linux (webkit2gtk) with Windows modules stubbed. Windows-only crates (`wmi`, `windows-*`, `nvml-wrapper`) compile only for the Windows target. | Tauri v2 cross-platform; windows-rs is Windows-only |
| 3 | **Cross-compile Windows builds from Linux with `cargo-xwin`** (`x86_64-pc-windows-msvc`). NSIS installer output. **Real builds/signing/tests go through GitHub Actions Windows runners** — no QEMU/Wine dependency for correctness. | Tauri docs: "Experimental: Build Windows apps on Linux and macOS"; codejam.info guide 2026 |
| 4 | **Privilege model: single elevated process for MVP, helper service later.** Registry (HKLM), services, power schemes, netsh, and cross-process priority all require admin. `requireAdministrator` manifest for v1; document the Process-Lasso-style service split as the production architecture. | Win32 API requirements |
| 5 | **DB: `rusqlite` directly in the Rust engine layer** (already in scaffold, v0.40.2). The official `tauri-plugin-sql` uses sqlx (heavier); the rusqlite fork (`tauri-plugin-rusqlite2`) is a 7-star community fork. Keep DB fully backend-side, expose typed Tauri commands. | tauri-plugin-sql docs; crate ecosystem research |
| 6 | **GPU monitoring: NVML (NVIDIA) + PDH performance counters (all GPUs) + DXGI (fallback).** WMI does **not** expose real GPU utilization. NVML gives temp/power/clocks/fan for NVIDIA; `\GPU Engine(*)\Utilization Percentage` (PDH, `pdh.dll`) gives per-process & per-engine utilization like Task Manager. AMD telemetry: PDH + WMI `Win32_VideoController`; optional ADL for temps. | MS Q&A "GPU Utilization"; tasksmack#383; NVIDIA docs |
| 7 | **Benchmark: bundle Intel PresentMon v2.3.1 console app** (MIT), run per-process capture, parse CSV. Compute avg FPS + **P1/P0.1 of frame time** for 1% lows (PresentMon 2.0 percentiles are NOT context-aware — compute manually). | PresentMon docs; GameTechDev issue #219 |
| 8 | **No fake boosts.** The plan's list of "optimizations" is curated by *measured evidence*: VBS/Memory Integrity off (biggest single lever, 5-15% in some titles), GameDVR off, HAGS on (latency, not FPS), power plan, shader cache management, process priority/affinity, DNS. TCP/IP registry tweaks are mostly marginal/placebo on modern Windows and are marked as such. | Windows Central 2025-08-14; IQON 2026-02-17; guru3D threads |
| 9 | **Snapshots are JSON + reversible change records**, stored in `C:\ProgramData\Optix\Snapshots\<id>\` per spec, plus optional System Restore point (`SRSetRestorePoint`) as an extra safety net before destructive ops. | Spec Phase 2 |
| 10 | **Elevated operations run on a background thread with progress events** (Tauri event system), never on the main/UI thread. Async commands + `tauri::async_runtime`. | Tauri v2 patterns |

---

## 2. Platform Reality Check

### Windows 10
- **Support ended October 14, 2025.** No security updates (consumer ESU free through Oct 2026, then paid).
- Still run by a large minority of gamers (~30%+ of Steam per mid-2026 surveys). Optix should run on it, but it is **not** the optimization target.
- Missing vs Win11: hybrid-CPU (P/E core) scheduler optimizations, DirectStorage, Auto HDR, windowed-game optimizations. **All APIs Optix needs exist on Win10.**

### Windows 11 (primary target, 25H2+)
- 2025-2026 independent benchmarks (TechSpot 14 games, Windows Central 7 games, HD Opti 23H2/25H2): **Win11 ≥ Win10 for gaming once defaults are fixed.** Out of the box it's often *slower* due to: Memory Integrity (VBS) on by default, Game Bar/GameDVR, telemetry, bloatware, Balanced power plan.
- This is exactly Optix's value proposition: a *reversible* pass that fixes the defaults, measured by the built-in benchmark.

### What actually moves FPS (ranked by evidence)
1. **Disable Memory Integrity / VBS** (Win11 default) — up to 5-15% in some titles. Security trade-off → requires explicit user consent, reversible, restore point.
2. **Disable GameDVR / Xbox Game Bar background recording** — 200-400 MB RAM + 18-23 ms input latency; CPU spikes on mid/low-end.
3. **Power plan** (High Performance / Ultimate Performance or cloned Optix plans; processor min/max state).
4. **GPU driver settings**: Power management mode → Prefer Maximum Performance; shader cache size ↑; (per-game: Reflex/LLM).
5. **Process priority/affinity during gaming** (above-normal for game, background processes lower). Never REALTIME.
6. **Shader cache management** — fixes stutter after driver/game updates; raising cache limit reduces mid-game recompilation stutter.
7. **HAGS** — ±2-3% FPS, main benefit is 1-3 ms latency + required for Reflex/frame-gen. Requires restart. Reversible.
8. **Cleanup of temp/junk** — no FPS gain, but free space + less disk churn (helps SSDs that are >90% full).
9. **TCP/IP tweaks** — *mostly placebo* on modern Windows; real wins come from DNS selection, driver updates, and wired connection. Marked "experimental" in UI.

---

## 3. Technology Stack (Validated Versions)

### Existing scaffold (already initialized in `/home/e4crypt3d/Documents/OptiX`)
| Component | Version (current) | Notes |
|-----------|-------------------|-------|
| Tauri | **2.11.x** (Jul 2026) | MSRV 1.85; `tauri` crate 2.11.5 |
| React | 19.1.0 → **19.2.8** (Jul 2026) | Upgrade pin |
| TypeScript | ~5.8.3 | |
| Tailwind CSS | **4.3.x** | v4 supported by shadcn/ui since Feb 2025 |
| Vite | 7.0.4 | |
| @tauri-apps/api + cli | ^2 (2.11.x) | |
| sysinfo | **0.39.6** | CPU/RAM/disks/networks/processes |
| tokio | 1.53.1 (full) | async runtime |
| rusqlite | **0.40.2** (bundled) | SQLite, no system dep |
| windows-sys | **0.61** | feature-gated Win32 bindings |
| winreg | 0.56.0 | registry access |
| serde / serde_json / anyhow / thiserror | 1 / 1 / 1.0.104 / 2.0.20 | |
| recharts | 3.10.1 | dashboard charts (present) |

### To add
| Crate | Version | Purpose |
|-------|---------|---------|
| `wmi` | 0.17.x | WMI queries (Win32_VideoController, MSFT_PhysicalDisk, thermal zones) |
| `nvml-wrapper` | 0.11.x | NVIDIA GPU telemetry (runtime-loaded via libloading) |
| `windows` (meta crate) | 0.61 (match windows-sys) | COM/Power/EventLog/Debug APIs with feature flags |
| `windows-version` | ^0.1 | `windows-version` crate — accurate OS version |
| `vdf` (or hand-rolled) | latest | Parse Steam `libraryfolders.vdf` / `appmanifest_*.acf` |
| `zip` | ^2 | `CrashReport.zip` generation |
| `hickory-resolver` | ^0.24 | DNS benchmark (or hand-rolled UDP DNS query — simpler, fewer deps) |
| `pdh` via windows-sys | 0.61 (Win32_System_Performance) | per-process/per-engine GPU utilization counters |
| `adlx` (or FFI) | latest (v2) | AMD ADLX telemetry — runtime-loaded, replaces legacy ADL |
| IGCL via FFI (`igcl.dll`) | latest (v2) | Intel Arc / Iris Xe telemetry (power/thermal/freq) |
| `dialoguer`-like / Tauri dialog + opener | present (plugin-opener) | file pickers for game paths |

> **Do not add:** sqlx (tauri-plugin-sql), heavy GUI libs, an external AI SDK for Phase 12 (rule-based first).

---

## 4. Development Environment: Linux → Windows

### Toolchain (on this Linux machine)
```bash
# Windows MSVC target
rustup target add x86_64-pc-windows-msvc
cargo install --locked cargo-xwin

# llvm-rc for the Windows resource file (icon/manifest)
sudo apt install lld llvm        # Ubuntu/Debian

# Build Windows installer from Linux
npm run tauri build -- --runner cargo-xwin --target x86_64-pc-windows-msvc
# Output: src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/Optix_0.1.0_x64-setup.exe
```
- `XWIN_CACHE_DIR` to share the downloaded Windows SDK across projects.
- **Signing**: Tauri only knows `signtool.exe` (Windows). From Linux use `osslsigncode` with a code-signing cert (EV recommended to avoid SmartScreen warnings); otherwise document the warning.
- **Caveat**: dev-mode `npm run tauri dev` on Linux runs a *Linux* build (webkit2gtk) — Windows-only modules must be `#[cfg(windows)]`. Windows integration is developed via: (a) unit tests for pure logic on Linux, (b) **GitHub Actions `windows-latest`** workflow for compile+test+smoke of the real Windows binary, (c) a local Windows VM (VirtualBox/Hyper-V) or `wine` smoke test (limited: WMI/powrprof/NVML won't fully work under Wine).

### CI workflow (recommended, per Tauri docs "GitHub Actions")
- `tauri-apps/tauri-action` on `windows-latest` → builds NSIS/MSI, attaches to releases.
- Matrix: `windows-latest` (release builds + integration tests). Linux job only for `cargo test` of engine logic.

### Config note
- Set `"identifier": "com.e4crypt3d.optix"` (already set) and `"bundle.targets": ["nsis", "msi"]`.
- Add `"windows": [{ "title": "Optix", "width": 1280, "height": 800 }]`.
- App manifest: `requestedExecutionLevel: requireAdministrator` (v1).

---

## 5. Architecture & Privilege Model

```
optix/
├── src/                    # React 19 + TS frontend (Vite, Tailwind v4, shadcn/ui)
├── src-tauri/
│   ├── src/
│   │   ├── main.rs         # Tauri builder, plugins, elevation, event loop
│   │   ├── lib.rs          # module tree + command registration
│   │   ├── commands/       # system.rs, processes.rs, storage.rs, power.rs,
│   │   │                   # network.rs, services.rs, snapshot.rs, gpu.rs
│   │   ├── engine/         # optimizer.rs, rollback.rs, benchmark.rs, recovery.rs,
│   │   │                   # diagnostics.rs (Phase 12)
│   │   ├── models/         # hardware.rs, snapshot.rs, optimization.rs
│   │   ├── db/             # sqlite.rs (schema + migrations)
│   │   └── win/            # cfg(windows)-only: powrprof.rs, services.rs, pdh.rs,
│   │                       # nvapi.rs, eventlog.rs, registry.rs, appx.rs
│   └── capabilities/       # Tauri permissions
```

### Privilege strategy (v1 → v2)
- **v1 (MVP)**: Single process, `requireAdministrator` manifest. Everything (UI + system work) runs elevated. Simple, honest. Downsides: UI runs as admin, no background agent.
- **v2 (production)**: Split into:
  - `OptixUI.exe` — normal user, webview frontend.
  - `OptixService` (Windows service, runs as SYSTEM, authored via `windows-services` crate) or a named-pipe RPC worker. UI talks to service via localhost/named pipe; service performs privileged ops; UI polls via Tauri events.
  - Game-mode watcher = service (starts on boot, low footprint).
- All destructive ops go through the **Rollback engine** regardless of model.

### Concurrency rules
- Blocking Win32 calls (registry, services, powrprof, PDH) → `spawn_blocking` / background thread; progress via `AppHandle.emit("optix://progress", ...)`.
- One monitoring sampler per subsystem (CPU/RAM/GPU/network) on a shared ticker (default 1 s, benchmark 100 ms).
- Tauri command signatures: `async fn` returning `Result<T, OptixError>` with `thiserror` typed errors serialized to the frontend.

---

## 6. Database Schema

DB file: `C:\ProgramData\Optix\optix.db` (created by service/elevated process; engine opens it read-write; UI never touches it directly). Migrations via `rusqlite` + `user_version` pragma.

```sql
-- Hardware history (dashboard trend lines, Phase 1/10)
CREATE TABLE hardware_history (
  id INTEGER PRIMARY KEY,
  ts INTEGER NOT NULL,            -- unix ms
  cpu_usage REAL, cpu_temp REAL,   -- temp nullable (sensor dependent)
  ram_used_mb INTEGER, ram_total_mb INTEGER,
  gpu_usage REAL, gpu_temp REAL, gpu_vram_mb INTEGER, gpu_power_w REAL,
  disk_used_mb INTEGER, disk_total_mb INTEGER,
  net_down_bps INTEGER, net_up_bps INTEGER,
  fps REAL, frame_time_ms REAL     -- populated during benchmark
);

-- Snapshots (Phase 2)
CREATE TABLE snapshots (
  id TEXT PRIMARY KEY,             -- uuid
  name TEXT, reason TEXT,
  created_at INTEGER, restored_at INTEGER,
  status TEXT                      -- active | restored | deleted
);
CREATE TABLE changes (
  id INTEGER PRIMARY KEY,
  snapshot_id TEXT NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
  domain TEXT NOT NULL,            -- registry|power|service|network|startup|process|file
  location TEXT NOT NULL,          -- exact key/name/path
  kind TEXT NOT NULL,              -- set|delete|start|stop|disable|kill|replace
  old_value TEXT, new_value TEXT,
  old_json TEXT, new_json TEXT,    -- structured payloads (service config, power idx, cmd)
  applied_at INTEGER, verified INTEGER DEFAULT 0,
  rolled_back INTEGER DEFAULT 0
);

-- Optimization profiles (Phase 5/9)
CREATE TABLE profiles (
  id INTEGER PRIMARY KEY,
  name TEXT UNIQUE, kind TEXT,     -- power | network | gaming | composite
  json TEXT NOT NULL               -- full profile payload
);

-- Games (Phase 9)
CREATE TABLE games (
  id INTEGER PRIMARY KEY,
  name TEXT, launcher TEXT,        -- steam|epic|battlenet|riot|xbox|gog|manual
  app_id TEXT, install_path TEXT, executable TEXT,
  last_played INTEGER, detected_at INTEGER
);
CREATE TABLE game_profiles (
  game_id INTEGER PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
  cpu_priority TEXT, affinity_mask TEXT, power_profile TEXT,
  network_profile TEXT, cleanup_bg INTEGER DEFAULT 0, gpu_profile TEXT,
  enabled INTEGER DEFAULT 1
);

-- Benchmarks (Phase 10)
CREATE TABLE benchmarks (
  id INTEGER PRIMARY KEY,
  game_id INTEGER REFERENCES games(id), game_name TEXT,
  started_at INTEGER, duration_ms INTEGER,
  avg_fps REAL, p1_fps REAL, p01_fps REAL,
  avg_frame_time_ms REAL, p95_frame_time_ms REAL,
  cpu_avg REAL, gpu_avg REAL, ram_avg_mb REAL, latency_ms REAL,
  config_hash TEXT,                -- snapshot of applied optimizations
  csv_path TEXT                    -- PresentMon raw CSV location
);

-- Crash reports (Phase 11)
CREATE TABLE crash_reports (
  id INTEGER PRIMARY KEY,
  detected_at INTEGER, app TEXT, pid INTEGER,
  event_id INTEGER, module TEXT, exception_code TEXT,
  wer_report_path TEXT, minidump_path TEXT, report_zip_path TEXT
);
```

---

## 7. Dashboard Specification

Layout (shadcn/ui, dark theme, OKLCH palette): sidebar (Dashboard, Scanner, Cleanup, Processes, Power, Startup & Services, Network, GPU, Game Profiles, Benchmarks, Snapshots/Rollback, Crash Reports, Settings).

### 7.1 Live telemetry panel (1 s refresh, benchmark 100 ms)
- **CPU**: per-core + total usage, frequency, top-5 process consumers. Temperature where a sensor is available (WMI thermal zone; optional LibreHardwareMonitor note).
- **RAM**: used/total, top-5 consumers.
- **GPU** (per adapter): utilization (3D + video engines), dedicated VRAM used/total, shared memory, temperature, fan %, power draw W, clocks (core/mem MHz), P-state, driver version. Per-process GPU usage from PDH.
- **Disk**: per-disk used/free, SSD/HDD type, health status (MSFT_PhysicalDisk), top I/O processes, and **free-space warning when <10-15%** (SSDs slow down).
- **Network**: down/up bps, ping to gateway, packet loss (ICMP probe), DNS latency.
- **Bottleneck meter**: CPU% vs GPU% during a session (which one saturates first).

**Counter sources (PDH, English names via `PdhAddEnglishCounterW` — sysinfo already uses this internally on Windows):**

| Metric | Counter path | Notes |
|--------|--------------|-------|
| CPU total | `\Processor(_Total)\% Processor Time` | or `GetSystemTimes` deltas |
| CPU per-process | `Process(*)\% Processor Time` | >100% on multicore → divide by core count or use `GetProcessTimes` |
| RAM available | `\Memory\Available MBytes` | risk when < 5-10% of total RAM |
| Memory pressure | `\Memory\Pages/sec`, `\Memory\Committed Bytes` | hard page faults = swapping |
| Disk latency | `\PhysicalDisk(*)\Avg. Disk sec/Read` + `Write` | <10 ms good, >50 ms bottleneck |
| Disk queue | `\PhysicalDisk(*)\Avg. Disk Queue Length` | correlate with latency |
| Network | `\Network Interface(*)\Bytes Total/sec`, `\Output Queue Length`, `\Packets Received Errors` | queue/errors should be 0 |
| GPU (per-process/per-engine) | `\GPU Engine(pid_<pid>_<eng>*)\Utilization Percentage` | sum 3D/Compute/VideoEncode/VideoDecode |
| Process working set / handles | `Process(*)\Working Set`, `Process(*)\Handle Count` | leak detection |

PDH gotchas: counter names are localized → always `PdhAddEnglishCounterW`; rate counters need two samples to mean anything; instances carry `#<pid>` suffixes (resolve via `PdhExpandWildCardPath`). WMI (`Win32_PerfFormattedData_*`) rounds to integers and is far more CPU-expensive — reserve WMI for static data (OS version, total RAM, serials, driver versions).

### 7.2 GPU & Cache panel (research-complete)
**Adapters & driver:**
- DXGI enumeration (`IDXGIFactory1::EnumAdapters`) for vendor/device name, video memory, driver version + NVML enrichment for NVIDIA.
- NVIDIA: `nvml-wrapper` → driver version, temp, power, clocks, fan, utilization, per-process VRAM.
- AMD: PDH utilization + WMI `Win32_VideoController.DriverVersion`; optional ADLX binding for temp/fan/clocks (Phase 8). Intel Arc/Iris Xe via IGCL.

**Cache inventory (sizes computed by recursive walk, safe-mode = count only):**

| Cache | Location | Notes |
|-------|----------|-------|
| NVIDIA DXCache | `%LOCALAPPDATA%\NVIDIA\DXCache` | DirectX shader cache |
| NVIDIA GLCache | `%LOCALAPPDATA%\NVIDIA\GLCache` | OpenGL shader cache |
| NVIDIA ComputeCache | `%LOCALAPPDATA%\NVIDIA\ComputeCache` | compute shaders |
| NVIDIA NV_Cache | `%LOCALAPPDATA%\NVIDIA Corporation\NV_Cache` and `C:\ProgramData\NVIDIA Corporation\NV_Cache` | legacy/global |
| NVIDIA PerDriverCache | `%LOCALAPPDATA%\NVIDIA\PerDriverCache\DXCache` | newer driver layout |
| **NVIDIA cache size setting** | NVAPI DRS `PS_SHADERDISKCACHE_MAX_SIZE` (global profile) | reads the configured limit (Off/5GB/10GB/100GB/unlimited); write via NVAPI or document manual NVCP change |
| AMD DxCache | `%LOCALAPPDATA%\AMD\DxCache` (also `DxcCache`, `VkCache`) | |
| **AMD ShaderCache mode** | `HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\000N\UMD` → `ShaderCache` REG_BINARY | `31 00` = AMD Optimized, `32 00` = Always On. NOTE: `0000` may be `0001+` after driver reinstalls — detect the live adapter key via Device Manager GUID lookup. |
| DirectX shader cache | `%LOCALAPPDATA%\D3DSCache` | also cleanable via Disk Cleanup "DirectX Shader Cache" |
| Steam shader cache | `steamapps\shadercache\<appid>\` per library | per-game sizes listed |

**Cache actions (all snapshot-first):**
- "Clear caches" → size preview → confirmation → delete → verify → record. **Warn**: first game launch after clearing will stutter while shaders rebuild (expected, not a failure).
- "Set NVIDIA shader cache size" (per user's driver) — write via NVAPI DRS if supported; otherwise open the setting in NVIDIA App/NVCP.
- "Set AMD shader cache Always On" — write `32 00` to the UMD key (with backup + rollback).

**Gaming toggles status card (read + reversible set):**
- HAGS: `HKLM\SYSTEM\CurrentControlSet\Control\GraphicsDrivers\HwSchMode` (2=on, 1=off, 0=driver default). Requires WDDM 2.7 GPU; **requires restart** to apply; note "mostly latency, ±2-3% FPS".
- GameDVR/Game Bar: `HKCU\System\GameConfigStore\GameDVR_Enabled=0`, `HKCU\Software\Microsoft\Windows\CurrentVersion\GameDVR\AppCaptureEnabled=0`.
- Memory Integrity / VBS: read `HKLM\SYSTEM\CurrentControlSet\Control\DeviceGuard\Scenarios\HypervisorEnforcedCodeIntegrity\Enabled` (1 = on). Disabling requires explicit consent + restart + restore point. *(Biggest single FPS lever on Win11.)*
- Game Mode: `HKCU\Software\Microsoft\GameBar\AutoGameModeEnabled` + allow/deny list.
- Active power plan GUID + name (PowrProf).
- Monitor refresh rate (`EnumDisplaySettingsW` / DXGI output desc).

### 7.3 Optimizations overview card
List of known optimization states (VBS, GameDVR, HAGS, power plan, shader cache size, DNS, startup bloat count, background services flagged) → each with **current state, measured-impact note, risk level, one-click apply/revert**, and a link to benchmark before/after.

---

## 8. Phase 0 — Foundation

**Goal:** working shell, CI, DB, error plumbing.

1. Update pins: React 19.2.x, Tailwind v4.3.x, `npm i`; add shadcn/ui (`npx shadcn@latest init`, use `sonner` for toasts — `toast` is deprecated upstream).
2. Add Rust deps above; keep `windows-sys` and `windows` at 0.61.
3. `src-tauri/src/win/` module tree with `#[cfg(windows)]` gating; Linux stubs return `Err(OptixError::UnsupportedPlatform)`.
4. `db/sqlite.rs`: open/migrate, seed helper, WAL mode.
5. `models/`: `HardwareInfo`, `GpuInfo`, `Snapshot`, `ChangeRecord`, `OptimizationProfile`, `BenchmarkResult`, `Game`, `CrashReport` (serde structs).
6. Error type: `OptixError` (thiserror) → `#[serde]` enum serialized to frontend.
7. GitHub Actions: `windows-latest` build+test; Linux `cargo test` job.
8. Elevation bootstrap: relaunch with `ShellExecuteW` "runas" if not elevated; `IsUserAnAdmin()` check; manifest `requireAdministrator`.

**Done when:** `npm run tauri dev` runs the shell on Linux; `tauri build --runner cargo-xwin` produces a signed-capable NSIS installer in CI; DB migrations run.

---

## 9. Phase 1 — System Scanner

### Hardware (source per item)
- **CPU model/cores/vendor/frequency**: `sysinfo` (`System::cpus()`, `Cpu::brand()`, physical/logical via `num_cpus` or WMI `Win32_Processor`).
- **GPU + driver**: DXGI enumeration for all adapters + `wmi` `Win32_VideoController` (DriverVersion, AdapterRAM, VideoModeDescription); NVIDIA enrichment via NVML. Display refresh rate via `EnumDisplaySettingsW`/DXGI.
- **RAM**: `sysinfo` total/used + `Win32_PhysicalMemory` for module layout (optional).
- **Storage**: `sysinfo` disks (capacity, free) + **`MSFT_PhysicalDisk`** (WMI, `root\Microsoft\Windows\Storage`) → `MediaType` (SSD/HDD/NVMe), `HealthStatus`, `BusType`, `Size`. This one query solves both "SSD/HDD type" and "storage health".
- **Motherboard/BIOS** (optional): `Win32_BaseBoard`, `Win32_BIOS`.

### Software
- **Windows version**: `windows-version` crate (`OsVersion`), build number, edition (`Win32_OperatingSystem.Caption`). Distinguish Win10 vs Win11 builds (Win11 = build ≥ 22000).
- **Installed games**: see Phase 9 launcher detection (shared module).
- **Running processes + startup apps**: `sysinfo` processes; startup enumeration shared with Phase 6.

### Scanner UX
- "Scan" runs each subsystem on background tasks with progress events; results cached in memory + `hardware_history` samples started on dashboard load.
- Never blocks UI; scanner results shown as cards with raw values + health badges (OK/WARN/CRIT thresholds).

---

## 10. Phase 2 — Snapshot & Recovery Engine  ⭐ core

Location per spec: `C:\ProgramData\Optix\Snapshots\<snapshot-id>\`

```
snapshot-id/
├── system.json        # hardware + OS fingerprint (detect changes between snapshots)
├── registry.json      # all keys touched by Optix, with old values (export via RegQuery)
├── services.json      # start types + running states of every touched service
├── power.json         # active scheme GUID + every AC/DC value index touched
├── network.json       # netsh global output + dynamic ports + interface DNS + MTU
├── startup.json       # Run keys/Startup folder entries + scheduled task states
├── process.json       # priority/affinity of processes we modify
├── gpu.json           # HAGS/GameDVR/VBS/GameConfigStore values + NVAPI profile blob
├── changes.json       # append-only journal of {type, location, old_value, new_value, timestamp}
└── timestamp.json
```

### Change record (spec format, extended)
```json
{
  "id": "uuid",
  "type": "registry.set",
  "location": "HKLM\\SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers\\HwSchMode",
  "old_value": {"kind": "DWORD", "value": 1},
  "new_value": {"kind": "DWORD", "value": 2},
  "timestamp": 1780000000000,
  "verified": true,
  "rollback_command": "registry.set"
}
```

### Rollback engine (`engine/rollback.rs`)
1. `snapshot.create(reason, domains)` — captures JSON files above.
2. `change.apply()` — perform op, then **verify** (re-read value; for services query state; for network re-run `netsh int tcp show global` and compare).
3. On any **verification failure → automatic emergency rollback** of that change (restore old value; if restore fails, mark snapshot `dirty` and surface a prominent banner + suggest System Restore point).
4. `snapshot.restore(id)` — apply all changes in **reverse order** with verification per step.
5. `snapshot.diff(a, b)` — structural comparison of the JSON files → human-readable "what changed" report.
6. Retention: keep last N (default 20) snapshots; oldest auto-deleted after user confirmation (configurable). Manual delete with warning.
7. Before destructive/cleanup ops, optionally create a **System Restore point** (`SRSetRestorePoint`, `rstrtmgr`) — requires enabling System Protection; try, and continue if unavailable.

### Safety net
- `C:\ProgramData\Optix\Snapshots` is ACL'd to `SYSTEM` + `Administrators` only.
- Every snapshot gets a manifest hash; rollback refuses to run against a tampered snapshot.

---

## 11. Phase 3 — System Cleanup

### Scan categories (path → safety level)
| Category | Locations | Safety |
|----------|-----------|--------|
| User temp | `%TEMP%`, `%LOCALAPPDATA%\Temp` | SAFE (skip in-use files) |
| Windows temp | `C:\Windows\Temp` | SAFE |
| Browser caches | Edge/Chrome `%LOCALAPPDATA%\*\Cache\*`, `Code Cache`; Firefox `%LOCALAPPDATA%\Mozilla\*\cache2` | SAFE |
| GPU shader caches | NVIDIA DXCache/GLCache/ComputeCache/NV_Cache; AMD DxCache/DxcCache/VkCache; `%LOCALAPPDATA%\D3DSCache` | SAFE (rebuild on next game launch — warn) |
| Crash dumps | `%LOCALAPPDATA%\CrashDumps`, `C:\Windows\Minidump` (keep newest) | SAFE |
| Update leftovers | `C:\Windows\SoftwareDistribution\Download` (only via Disk Cleanup semantics), delivery optimization cache | CAUTION (admin; skip if `wuauserv` mid-update) |
| Application logs | `%LOCALAPPDATA%\*\Logs\*.log` (age > 30 d) | CAUTION |
| Recycle Bin (optional, off by default) | `$Recycle.Bin` via SHQueryRecycleBin/EMPTYFLAG | CAUTION |

### NEVER touch (hard-coded deny list, unit-tested)
`System32`, `WinSxS`, `System32\DriverStore`, `C:\Windows\Installer`, `Program Files` tree, `AppData\Roaming\Microsoft\Windows\Start Menu` (unless Phase 6), live game install dirs, `C:\Windows\WinSxS\*`. Also never delete the folder roots themselves — only contents.

### Bloatware (AppX / MSIX packages) — research-complete
Dedicated module (`commands/bloatware.rs`), **allowlist model** (PDQ practice): never blanket-remove; ship a curated removal list + a protected allowlist, both unit-tested and versioned per OS SKU.

- **Enumerate**: WinRT `PackageManager::FindPackagesForUser` (`windows` crate `Management::Deployment`) — name, version, architecture, install location. Pragmatic fallback: invoke `powershell -Command "Get-AppxPackage | ConvertTo-Json"` and parse (identical data, slower).
- **Remove**: per-package confirm. Current user: `Remove-AppxPackage` (`PackageManager.RemovePackageAsync`). All users: `-AllUsers`. **Prevent reinstall for new users / after updates**: remove the *provisioned* package — `DISM /Online /Remove-ProvisionedAppxPackage /PackageName:"<fullname>"` (fallback: PowerShell `Get-AppxProvisionedPackage -Online`).
- **Protected allowlist (NEVER flagged — a gaming tool must not break these)**: Calculator, Notepad, Paint, Photos, Store, Terminal, Camera, **Xbox Gaming Overlay, Xbox Game Callable UI, Xbox Identity Provider, Xbox TCUI** (all Xbox/gaming-adjacent), `Microsoft.net.*`, WindowsAppRuntime. `Microsoft.Advertising.Xaml` = CAUTION (some apps depend on it), not SAFE.
- **Removal candidates (SAFE, per-OS versioned)**: Clipchamp, Solitaire Collection, News, Weather, Get Help, Feedback Hub, Skype, Mixed Reality Portal, Maps, People, To Do, Office Hub, Xbox Console Companion (Win11: gone anyway), social/promo (Facebook/Instagram/Twitter/LinkedIn/WhatsApp/Telegram), streaming (Netflix/Disney+/Spotify/TikTok), shopping (Amazon/Temu/eBay/Booking), OEM trials (McAfee; **Dolby = CAUTION** — may be needed for spatial audio).
- **Reappearance**: feature updates re-provision packages → re-scan after every feature update; removing the provisioned package prevents most reinstallations.
- **25H2 native option**: "Remove default Microsoft Store packages" (Windows Components → Package Deployment GPO; Pro/Enterprise GPE only) — detect GPE availability and offer "let Windows do it natively" when possible.
- **Consumer/telemetry features** (registry, reversible, CAUTION): `HKLM\SOFTWARE\Policies\Microsoft\Windows\CloudContent` → `DisableWindowsConsumerFeatures=1`; News/Interests + Spotlight toggles.
- **WinSxS safe cleanup** (optional, admin): `dism /online /cleanup-image /startcomponentcleanup` — Microsoft-sanctioned, frees several GB; run as subprocess with streamed output; never `resetbase` unless explicitly asked.
- Rollback: snapshot `appx.json` (installed + provisioned lists) before any removal → reinstall via `Add-AppxPackage` (Store) / `Add-AppxProvisionedPackage` (new users).

### UX
- Scanner computes: category, total size, safety level, "expected rebuild" note.
- User selects categories → creates lightweight snapshot → deletes → verifies freed space (re-scan affected dirs) → writes changes to snapshot.
- "What can I delete" ranking by size×safety.

---

## 12. Phase 4 — Process & RAM Management

### Analyzer (`commands/processes.rs` + sysinfo)
Per-process: CPU %, RAM, disk I/O, network (sysinfo provides `Process::disk_usage()`, network per-process on Windows via `GetProcessIoCounters`-style counters), and **GPU usage per process via PDH `\GPU Engine(pid_<pid>_...)\Utilization Percentage`** (sum engines 3D/Compute/VideoEncode/VideoDecode).

**Win32 API reference (windows-sys `Win32::System::Threading` + `Win32::System::Diagnostics::ToolHelp`):**

| Concern | API | Notes |
|---|---|---|
| Enumerate | `CreateToolhelp32Snapshot` + `Process32FirstW/NextW` | pid, parent, threads, path; `PROCESS_QUERY_LIMITED_INFORMATION` suffices for reads |
| CPU per-process | `GetProcessTimes` deltas | exact Task-Manager-style 0-100%; PDH `Process(*)\% Processor Time` exceeds 100% on multicore (sum of cores) → prefer GetProcessTimes (Splunk `useWinApiProcStats` practice) |
| CPU system | `GetSystemTimes` | idle/kernel/user deltas |
| RAM system | `GlobalMemoryStatusEx` | available/commit/pagefile |
| RAM per-process | `GetProcessMemoryInfo` (PSAPI) | working set, private bytes |
| Disk I/O | `GetProcessIoCounters` | read/write byte counters |
| Priority | `Get/SetPriorityClass` | Idle=4, BelowNormal=6, Normal=8, AboveNormal=10, High=13, Realtime=24 (never) |
| Affinity | `Get/SetProcessAffinityMask` | hard mask; **prefer CPU Sets API** on hybrid CPUs (soft affinity, power-management-compatible, `SetProcessDefaultCpuSets`) |
| Suspend/Resume | `NtSuspendProcess/NtResumeProcess` (ntdll, undocumented) | used by Task Manager; needs `PROCESS_SUSPEND_RESUME`; caller must stay alive while target suspended |
| Modules/name | `EnumProcessModules` + `GetModuleFileNameExW` | |
| Network per-process | `GetExtendedTcpTable/GetExtendedUdpTable` | owner PID per socket |
| Elevation check | `OpenProcessToken` + `GetTokenInformation(TokenElevation)` | |
| Kill | `TerminateProcess` | only after user confirm; never SYSTEM-owned PIDs |

### Classification
- **REQUIRED**: System (`System`, `svchost` core set), Windows core, `explorer` (keep), Defender, current app.
- **SAFE**: user apps in allowlist (browsers, launchers, cloud sync, updaters) — user confirms.
- **UNKNOWN**: heuristic (unsigned, high RAM, no window, known bloat list) → treated as OPTIONAL, shown with flags.
- Kill only processes the user confirms; never kill SYSTEM-owned PIDs.

### Gaming mode (engine/optimizer.rs + watcher thread)
When a configured game process is detected (Phase 9 integration):
- Game process: `SetPriorityClass(ABOVE_NORMAL_PRIORITY_CLASS)` — **never REALTIME** (can freeze input/audio; spec compliance enforced + unit test).
- `SetProcessAffinityMask` to preferred cores (P-cores on Intel hybrid / CCD0 on Ryzen when beneficial — off by default, per-game).
- Background list: `SetPriorityClass(BELOW_NORMAL)` on chosen background processes.
- On game exit: restore all priorities/affinity (via snapshot).
- RAM: no aggressive "memory clean" voodoo — only surface top consumers + suggest closing SAFE processes. Optionally call `SetProcessWorkingSetSize` on idle background apps (documented as cosmetic on modern Windows).

---

## 13. Phase 5 — Power Management

### API (windows-sys `Win32::System::Power` / powrprof)
- `PowerGetActiveScheme(NULL, &guid)`, `PowerSetActiveScheme(NULL, &guid)`
- `PowerDuplicateScheme(NULL, base_guid, NULL, &new_guid)` — **create Optix plans by cloning** (never editing built-ins in place; rollback-safe)
- `PowerWriteACValueIndex / PowerWriteDCValueIndex(root, scheme, subgroup, setting, value)` then `PowerSetActiveScheme` to commit
- `PowerReadACValueIndex` for current values
- `PowerEnumerate` to list settings/aliases; `PowerDeleteScheme` for cleanup

### Optix power profiles (clones of built-ins)
| Profile | Base | Processor min/max state | PCIe ASPM | USB selective suspend | GPU preference |
|---------|------|------------------------|-----------|------------------------|----------------|
| Balanced Gaming | Balanced | 100%/100% on AC | Off (AC) | Off | Prefer Max Perf (NVIDIA) |
| Competitive Gaming | High performance | 100%/100% | Off | Off | Prefer Max Perf + LLM Ultra (optional) |
| Maximum Performance | Ultimate Performance (hidden — duplicate `e9a42b02-d5df-432d-aa00-6a11a9fd3e6e`) | 100%/100% | Off | Off | Prefer Max Perf |

### Setting GUIDs (well-known, used with the subgroups)
- Processor subgroup `54533251-82be-4824-96c1-47b60b740d00`:
  - min state `893dee8e-2bef-41e0-89c6-b55d0929964c`, max state `bc5038f7-23e0-4960-96da-33abaf5935ec`
  - idle disable `5d76a2ca-e8c0-402f-a133-2158492d58ad` (only for Competitive; with thermal warning)
- PCI Express subgroup `501a4d13-42af-4429-9fd1-a8218c268e20`: ASPM `ee12f906-d277-404b-b6da-e5fa1a576df5` (0 = off)
- USB subgroup `2a737441-1930-4402-8d77-b2bebba308a3`: selective suspend `48e6b7a6-50f5-4782-a5d4-53bb8f07e226`
- Display/GPU power preference lives in the **driver** (NVAPI PREFERRED_PSTATE, AMD power efficiency) — Phase 8, not powercfg.

### Rollback
- Snapshot captures active scheme GUID + all AC/DC indexes we wrote → restore = write originals back + `PowerSetActiveScheme` original. Profile application stores `profiles` row + change records.
- Laptops: write AC and DC values separately; never force DC to 100% (battery warning in UI).

---

## 14. Phase 6 — Startup & Service Manager

### Startup analysis sources
- Registry Run keys: `HKCU\...\CurrentVersion\Run`, `HKLM\...\Run`, `HKLM\...\Wow6432Node\Run`, both `RunOnce` (read-only).
- Startup folders: `shell:startup` + `shell:common startup` (`.lnk`, `.exe`, `.bat`).
- Scheduled tasks: Task Scheduler COM (`windows` crate `Win32::System::TaskScheduler`) or `schtasks /query /fo CSV /v` fallback; classify by author/action path.

### Services (windows-sys `Win32::System::Services`)
- Enumerate: `EnumServicesStatusExW` (all, state including stopped) → name, display name, start type, state, binary path, description.
- Operations: `ChangeServiceConfigW` (start type), `ControlService` (stop/start), `QueryServiceStatusEx`. Requires admin (service handle `SERVICE_CHANGE_CONFIG`).
- Never-flag list (unit-tested, hard-coded): `WinDefend`, `wscsvc`, `wuauserv`, `RpcSs`, `Dhcp`, `Dnscache`, `NlaSvc`, `EventLog`, `BFE`, `mpssvc`, `cryptsvc`, all `*Driver*`/`*Filter*` services, anything with `SERVICE_DRIVER` type.

### Classification
- **SAFE**: allowlist (e.g., `SysMain`/Superfetch — often flagged as RAM hog; mark "test with benchmark"), Xbox services (`XblAuthManager`, `XboxGipSvc`, `XblGameSave`, `XboxNetApiSvc`), telemetry (`DiagTrack` — note privacy/functionality trade-off), cloud-sync background agents.
- **UNKNOWN**: unknown publishers, high-RAM services, drivers — show path + signature status; default **no action**.
- **REQUIRED**: everything in the never-flag list above.
- Changes: snapshot `services.json` → apply → verify state change → rollback-capable.

### Windows Search index (WSearch) — dedicated toggle, research-complete
- Service: `WSearch` (SearchIndexer.exe). Disable = service start type → `HKLM\SYSTEM\CurrentControlSet\Services\WSearch` `Start` DWORD: `4` (Disabled) / `2` (Automatic) / `3` (Manual); runtime stop via `ControlService(SERVICE_CONTROL_STOP)` (windows-sys `Win32::System::Services`).
- **Impact honesty**: the indexer is designed to run only when the PC is idle/low-load — disabling mostly helps HDD/low-RAM PCs; on modern SSD systems it frees ~200-500 MB RAM and occasional disk wakeups. Cost: Start-menu search, Explorer search and Outlook search become slower; feature updates can re-enable it.
- **Middle-ground options offered first**: keep Classic (not Enhanced) index scope, exclude large folders (Steam/game libraries), relocate the index DB (`C:\ProgramData\Microsoft\Search\Data`) to a faster drive, enable indexer backoff.
- Rollback: restore `Start` DWORD + start service. Never delete the search index DB as "cleanup" while the service is running.

---

## 15. Phase 7 — Network Optimization

### Monitoring (real, measured — not placebo)
- Ping / packet loss / jitter: ICMP via `tokio` + `socket2` raw or `ping` crate; measure to gateway + configured game server.
- DNS latency per server: raw UDP DNS queries (A record for a fixed set of domains) with per-server timing — hand-rolled minimal DNS client (fewer deps than hickory; ~100 lines) or `hickory-resolver` configured per nameserver.
- Throughput: TCP download test to a fast CDN endpoint (optional, user-initiated).

### DNS benchmark (research-complete)
- **Server list (2026)**: Cloudflare `1.1.1.1`/`1.0.0.1`, Google `8.8.8.8`/`8.8.4.4`, Quad9 `9.9.9.9`, Control D `76.76.2.0`/`76.76.10.0`, NextDNS (custom config), Mullvad `dns.mullvad.net` (DoH/DoT), DNS4EU `86.54.11.100` (EU), ISP DNS (read from interface config).
- **Method**: raw UDP DNS queries (A record for a fixed domain set), N=20, report median/p95 — hand-rolled ~100-line client (fewer deps than hickory). Also ICMP-ping each resolver for an RTT baseline.
- **DoH/DoT note**: encrypted DNS adds ~5-15 ms per lookup (TLS/HTTPS handshake); pure speed → traditional port 53. Windows 11 22H2+ has native DoH in Settings — surface as a user choice, never silently pick.
- **Honest expectations (UI copy)**: DNS affects launcher boot, login, matchmaking lookups, CDN selection (EDNS Client Subnet), patch starts — **not in-game ping after the connection is established**.
- **Multi-resolver**: allow primary+secondary from different providers (failover), e.g. Cloudflare primary / Quad9 secondary.
- **Apply**: `netsh interface ip set dns <ifname> static <ip>` per adapter, **or** registry `HKLM\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\{guid}` → `NameServer`. Snapshot old value. After apply: `ipconfig /flushdns` (or `DnsFlushResolverCache`).
- **Verify**: re-run benchmark after apply + `Resolve-DnsName` check. Some ISP routers ignore client DNS overrides or transparently redirect port 53 → surface "router may override; check your router" when the benchmark shows no change.

### TCP analyzer (explicitly "experimental" in UI — research says marginal on modern Windows)
- Read current: `netsh int tcp show global`, `netsh int ipv4 show dynamicport tcp`, registry TCP params.
- Optional changes (all snapshot + revertible):
  - `netsh int tcp set global autotuninglevel=normal` (heuristics can still override — check `netsh int tcp show heuristics`)
  - `rsc=disabled`, `timestamps=disabled`, `nonsackrttresiliency=disabled`, `maxsynretransmissions=2`, `initialRto=300` (min)
  - `rss=enabled` (only on NICs that support it)
  - Registry `Tcpip\Parameters`: `TcpAckFrequency=1`, `TCPNoDelay=1` (Nagle off — only affects TCP, most games use UDP), `MaxUserPort=65534`, `TcpTimedWaitDelay=30`, `DefaultTTL=64` — **each shown with "impact: low/medium, may not help your game"**
- Reset: restore from snapshot (`netsh ... =default`, delete registry values).
- **Revert to defaults is one click**; the analyzer records before/after ping + packet loss so the user sees *whether it did anything*.

---

## 16. Phase 8 — GPU Management

### Detection & telemetry (NVIDIA / AMD / Intel — research-complete)
- **NVIDIA**: `nvml-wrapper 0.11` — driver version, name, temp, power, clocks, fan, utilization, VRAM, per-process memory. Runtime-loaded (`libloading`) — no build-time NVIDIA dependency. Optional NVAPI (`nvapi.dll`, FFI; current SDK release 590, Feb 2026, Win10+) for GPU thermal/clock/cooler interfaces + DRS below.
- **AMD**: PDH utilization + WMI (`Win32_VideoController`) for driver version. **v2 telemetry via ADLX (AMD Device Library eXtra, v1.5 Apr 2026)** — modern SDK replacing legacy ADL: `IADLXGPUMetrics` (utilization, VRAM, fan RPM + duty %, power, GPU/VRAM clocks), GPU tuning, VGM/memory info. Rust: `adlx` crate or FFI, runtime-loaded like NVML. Legacy ADL (`ADL2_Main_Control_Create`) only as fallback.
- **Intel**: IGCL (Intel Graphics Control Library — `intel/drivers.gpu.control-library`) — Control API + hardware monitoring/telemetry (power, thermal, frequency, EU activity) for Arc A/B-Series and Iris Xe; runtime-loaded `igcl.dll`. PDH `\GPU Engine` utilization works for Arc as well. (Intel Unified Telemetry is a separate profiling CLI — not embeddable.)
- Fallback (no vendor API): DXGI `IDXGIAdapter3::QueryVideoMemoryInfo` for memory budgets; no util/temp.

### Driver settings (reversible)
- **NVIDIA** via **NVAPI Driver Settings (DRS)** — `NvApiDriverSettings.h` IDs (NVIDIA exposes since R256):
  - `PREFERRED_PSTATE_ID` → `PREFERRED_PSTATE_PREFER_MAX` (Power management mode)
  - `PS_SHADERDISKCACHE_ID` / `PS_SHADERDISKCACHE_MAX_SIZE_ID` (shader cache on + size)
  - `PRERENDERLIMIT_ID` (max pre-rendered frames)
  - **Low Latency Mode is NOT supported through NVAPI** (confirmed by NVIDIA forum moderator) — surface as "set in NVIDIA App/NVCP" instruction, or warn before writing the profile manually.
  - Access pattern: `NvAPI_Initialize` → `NvAPI_DRS_GetSession` → `GetBaseProfile`/`GetProfile` → `Get/SetSetting` → `NvAPI_DRS_SaveSettings`. Bind via FFI (`nvapi.dll`) — small `nvapi.rs` module with the ~15 functions needed; snapshot the DRS profile blob before writes.
- **AMD**: registry `...\Control\Class\{4d36e968-...}\000N\UMD` `ShaderCache` `31→32 00`; power efficiency via Adrenalin settings → document-only in v1 (no supported public API for all settings; ADL exposes some).
- HAGS / GameDVR / VBS handled in Phase 7 dashboard card (registry, reversible).

### Per-game GPU profiles (feeds Phase 9)
- Create/apply DRS per-application profile (game exe path + settings) — this is exactly how NVCP per-game profiles work. Rollback = delete profile / restore blob.

---

## 17. Phase 9 — Game Profile System

### Launcher detection (all read-only, snapshot-safe)
| Launcher | Method |
|----------|--------|
| Steam | `HKLM\SOFTWARE\WOW6432Node\Valve\Steam` → `InstallPath`; parse `steamapps\libraryfolders.vdf` (VDF KeyValues) for all libraries; each `<lib>\steamapps\common\<name>` + `appmanifest_<id>.acf` (name, state). Use `vdf` crate or ~80-line parser. |
| Epic | `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests\*.item` (JSON: `DisplayName`, `InstallLocation`, `LaunchExecutable`) |
| Battle.net | `C:\ProgramData\Battle.net\Agent\agent.db` (SQLite: `tbl_battle_net_products`) + Uninstall registry keys |
| Riot | `HKLM\SOFTWARE\WOW6432Node\Riot Games\*` (VALORANT, LoL) |
| Xbox / Game Pass | MSIX enumeration (`Get-AppxPackage` equivalent via WinRT/registry `AppxAllUserStore`); game dirs under `C:\Program Files\WindowsApps` are ACL-gated → read metadata from registry, treat install-path as best-effort |
| GOG | `%PROGRAMDATA%\GOG.com\Galaxy\galaxy.db` |
| Manual | user picks exe |

### Profile model (per game)
```
cpu_priority (normal|above_normal|high)
affinity_mask (optional, hex)
power_profile (balanced_gaming|competitive|maximum|none)
network_profile (dns|tcp_experimental|none)
gpu_profile (nvidia_drs application profile: power mode, shader cache, LLM hint)
cleanup_bg (kill/background-lower SAFE list during play)
benchmark_enabled
```

### Game mode watcher
Background thread polls `sysinfo` processes (1 s); when a configured game starts → apply profile via Rollback engine (records each change); on exit → restore. Detect launcher-indirect games (Steam→game) via known exe names in the game's install dir (like SteamLibrarian's process-detection approach) with window-title fallback.

---

## 18. Phase 10 — Benchmark System

### Method (PresentMon-based, industry-standard)
1. Bundle `PresentMon64.exe` v2.3.1 (MIT) in `resources/` (run from app data; detect existing install).
2. User workflow: pick game profile → "Start benchmark" → Optix launches game (or attaches to process) → PresentMon captures ETW frame timing to CSV → app polls and stops capture after configured duration.
3. Analysis (parse CSV): `MsBetweenDisplayChange` (user-perceived frame pacing), `MsBetweenPresents`; compute:
   - avg FPS, **P1 frame-time → 1% low FPS, P0.1 → 0.1% low** (PresentMon 2.0 percentiles are per-column, not context-aware — compute inversions ourselves)
   - frame-time p95, dropped frames count
   - concurrently sample sysinfo + NVML/PDH → CPU/GPU/RAM/latency averages.
4. Before/After: run with baseline snapshot vs optimized snapshot; store `config_hash` = hash of applied change records so comparisons are meaningful; render comparison table + recharts frame-time graph.
5. Caveats surfaced in UI: benchmark the **same scene** (manual route), fixed duration, close background apps, 3-run minimum for statistical confidence.

---

## 19. Phase 11 — Crash Recovery

### Monitoring (background)
- Event Log (windows-sys `Win32::System::EventLog`: `EvtQuery` Application channel, `EvtNext`, `EvtRender` → XML): watch Event IDs
  - **1000** Application Error (faulting module + exception code, e.g. `c0000005`)
  - **1001** Windows Error Reporting (WER report folder + app)
  - **4101** Display driver stopped responding (TDR — GPU crash), **1014/1016** NVVLDMM events
- WER reports: `%LOCALAPPDATA%\Microsoft\Windows\WER\ReportArchive\AppCrash_*` + `ReportQueue`
- Minidumps: `%LOCALAPPDATA%\CrashDumps\*.dmp` (user apps), `C:\Windows\Minidump\*.dmp` (kernel/driver)
- Correlate with running-game session (Phase 9 watcher) → "Valorant crashed at 14:32" context.

### Report generation
- `CrashReport.zip` via `zip` crate: exported event-log XML for the crash window, WER `Report.wer` + metadata, newest minidump (copy, size-capped), Optix session log, hardware/snapshot state at crash time (`system.json`).
- UI: crash timeline, per-app crash count, driver/GPU errors highlighted with "check GPU drivers" recommendation; button to open report folder / share zip.

---

## 20. Phase 12 — AI Diagnostics (Optional)

- **No random changes.** Rule-based diagnostic engine (`engine/diagnostics.rs`) with confidence scoring:
  - High disk usage + `OneDrive.exe`/`Dropbox`/`msedge` I/O → "Cloud sync/background sync" cause, confidence %, recommendation "Pause sync during gaming".
  - High CPU + updater processes → recommendation to defer updates.
  - Low FPS + GPU < 60% + CPU 100% → "CPU bottleneck" → suggest closing background apps / power plan.
  - TDR events + GPU temps > 85 °C → thermal/driver suggestion.
  - Pagefile/commit pressure → RAM recommendation.
- Data sources: hardware_history, process samples, event log, benchmark rows.
- If a real LLM API is added later: feed the same evidence (never raw system dumps) with a strict prompt + "this is an AI suggestion" framing. Default: local rules only.

---

## 21. Windows 10 vs Windows 11 Matrix

| Concern | Win10 | Win11 | Optix behavior |
|---------|-------|-------|----------------|
| Support | Ended 2025-10-14 (ESU only) | Active (25H2) | Banner in UI on Win10 recommending upgrade |
| VBS/Memory Integrity | Off by default (usually) | On by default | Detect + optional reversible disable on Win11 |
| Hybrid CPU (Intel P/E, Ryzen CCD) | No scheduler awareness | Yes | Affinity presets only on Win11 for hybrid; manual on Win10 |
| GameDVR/Game Bar | Present | Present (more default on) | Same registry disable path |
| HAGS | 2004+ | Yes | Same `HwSchMode` registry; requires WDDM 2.7 |
| DirectStorage / Auto HDR | No | Yes | Display "unavailable" on Win10; no action |
| powercfg | Full | Full | Same PowrProf API; plan GUIDs/aliases differ per SKU → always use GUIDs, verify via `PowerEnumerate` |
| netsh TCP | Some params deprecated (Win8+) | Same | Same commands; prefer PowerShell cmdlets fallback (`Set-NetTCPSetting`) |
| WMI/DXGI/EventLog/PDH | Available | Available | Identical code paths |
| Scheduler/telemetry services | Different set | Different set | Classifier lists per-OS; SAFE/REQUIRED lists versioned |

---

## 22. Security Rules

1. **Never** make irreversible changes: every mutation creates a change record first.
2. **Never** auto-delete: cleanup requires explicit user confirmation with size preview.
3. **Never** permanently disable security features: VBS/Defender operations require opt-in consent, create restore point, and are fully reversible.
4. **Never** modify unknown registry keys without backup — the registry snapshot covers everything Optix touches; unknown-key edits are blocked by an allowlist.
5. **Never** REALTIME priority; never kill REQUIRED processes; never touch `System32/WinSxS/DriverStore/Windows\Installer`.
6. All snapshot/DB files: `SYSTEM`/`Administrators` ACL; zip reports exclude credentials; logs strip user paths that contain PII unless a crash report explicitly includes them.
7. Elevation is required for privileged ops and refused otherwise (`ERROR_ACCESS_DENIED` → friendly message, never silent retry loops).
8. Signed binaries (osslsigncode in CI); CSP set in `tauri.conf.json` (currently `null` — set a real CSP before release).

---

## 23. Risk Register & Honest Expectations

| Risk | Mitigation |
|------|------------|
| **"FPS booster" expectations** | Marketing + UI explicitly: reversible system management, benchmark-verified. Ship Phase 10 benchmark early. |
| VBS disable reduces security | Requires explicit consent, restore point, revert one click, warning text. Default OFF. |
| TCP tweaks = placebo for many users | Label "experimental", show before/after ping measurement, one-click revert. |
| `HwSchMode`/registry toggles reverted by OEM apps (documented: DPVR, MSI Center) | Verify after apply; report "overridden by another program" and revert attempt. |
| Shader cache clearing causes first-launch stutter | Clear warning + expected-rebuild note (this is normal). |
| WMI/DXGI data unavailable on some drivers | Graceful degradation; every field nullable with "unavailable" state. |
| Cross-compile breakage of Windows-only crates | CI windows-latest builds every PR; Linux unit tests cover engine logic only. |
| AMD temp/power telemetry limited in v1 | PDH util + VRAM always; NVML full on NVIDIA; ADLX (v1.5) optional v2. Intel Arc via IGCL. |
| Benchmark comparability (different scenes) | Fixed-duration captures, same-route guidance, 3-run minimum, config_hash. |
| SmartScreen on unsigned installers | EV cert + osslsigncode in CI; document first-run warning. |

---

## 24. Testing & QA Strategy

- **Unit tests (Linux CI, `cargo test`)**: rollback engine ordering + verification logic, change-record serialization, VDF parser, cleanup deny-list, service never-flag list, DNS median/p95 math, benchmark CSV parser, percentile inversion (P1 lows), registry path allowlist, safety classifier.
- **Integration tests (Windows CI, `cargo test --target x86_64-pc-windows-msvc` via `wine` runner where feasible / VM)**: powrprof read-only calls, PDH counter enumeration, Event Log query, startup enumeration, snapshot create/restore on a disposable VM snapshot.
- **Manual QA on real Windows (VM + a physical gaming PC)**: every phase's apply/revert cycle, elevation flows, benchmark before/after on 2 games, crash-report generation from an induced crash.
- **Visual QA**: shadcn/ui components + dashboard charts in dark mode; Tauri v2 window chrome.
- **Security review** before v0.1 release: capability permissions, CSP, no secret logging, zip path traversal guards in CrashReport.zip, path allowlist fuzzing.

---

## 25. Milestones

| Milestone | Scope | Exit criteria |
|-----------|-------|---------------|
| **M0 — Foundation** (1-2 wks) | P0: scaffold, deps, CI, DB, elevation, error types | Cross-compiled NSIS builds green in CI; shell runs on Linux + Win11 VM |
| **M1 — Scanner + Dashboard** (2-3 wks) | P1 + §7 dashboard (incl. GPU & cache panel) | Full telemetry + cache inventory live; data persisted |
| **M2 — Safety core** (2-3 wks) | P2 snapshot/rollback engine | Create/restore/diff/emergency-rollback tested end-to-end |
| **M3 — Cleanup + Processes** (2-3 wks) | P3, P4 | Safe cleanup + gaming mode apply/revert on a real PC |
| **M4 — Power + Startup/Services** (2-3 wks) | P5, P6 | Optix power profiles with rollback; classifier verified |
| **M5 — Network + GPU** (2-3 wks) | P7, P8 | DNS benchmark applies/best DNS; NVAPI DRS profile snapshot/restore; cache size controls |
| **M6 — Game profiles + Benchmark** (3-4 wks) | P9, P10 | Steam/Epic detection; PresentMon before/after report |
| **M7 — Crash recovery + polish** (2-3 wks) | P11, P12, release hardening | CrashReport.zip on induced crash; AI diagnostics v1; signing; SmartScreen docs |
| **v1.0 release** | — | Full safety guarantee demonstrated in release notes |

**Total realistic:** ~4-5 months part-time, ~2.5-3 months focused.

---

## 26. Source References

- Tauri v2: release page (2.11.x, Jul 2026), "Experimental: Build Windows apps on Linux/macOS" (cargo-xwin guide), distribute/windows-installer, plugin/sql, GitHub Actions pipelines, MSRV 1.85 (tauri-build 1.5.7-edition2024.0). Cross-compile walkthrough: codejam.info (2026-04), mobzystems blog (2025-11).
- React 19: changelog (19.2.8, 2026-07-21), versionlog.com/react/19.0. Tailwind v4 + shadcn/ui: ui.shadcn.com/docs/tailwind-v4, shadcn-ui/ui#6427/#6585, SO 79423511. Sonner toast deprecation note.
- windows-rs: github.com/microsoft/windows-rs (family: windows, windows-sys, windows-registry, windows-services, windows-version); Microsoft Learn "Rust for Windows, and the windows crate". WMI: crates.io wmi 0.17.x.
- sysinfo 0.39: lib.rs/crates/sysinfo, docs.rs (Cpu, Process, Disks, Networks, Components).
- Power: Microsoft Learn Power Scheme Management (PowerEnumerate/DuplicateScheme/SetActiveScheme/DeleteScheme), PowerGetActiveScheme, PowerWriteACValueIndex (powrprof.h), powercfg aliases caveat (learnmandu 2026-05).
- Processes: SetPriorityClass/SetProcessAffinityMask (Win32_System_Threading), NtQuerySystemInformation; sysinfo per-process CPU/disk/network; PDH GPU Engine counters (tasksmack#383, MS Learn "GPU activity monitoring").
- GPU: nvml-wrapper 0.11 (lib.rs; temp/power/clocks/fan/util, runtime-loaded), NVIDIA NVAPI DRS (NvApiDriverSettings.h — PREFERRED_PSTATE_ID, PS_SHADERDISKCACHE*, PRERENDERLIMIT_ID), NVIDIA forum: Low Latency Mode not supported via NVAPI (2023), NVIDIA KB 3130 (power management mode), shader cache locations (NVIDIA KB 5735, Gaijin KB, Tier1Settings 2026-07, blutrumpet 2026-04), AMD UMD ShaderCache registry (r/Amd z3i5yo, guru3d thread 24.6.1, MPO-GPU-FIX wiki), HAGS HwSchMode (NinjaOne 2026-05, Tech2Geek 2026-06, ServerFault 1121358), GameDVR GameConfigStore (evezone 2026-06), GPU utilization via WMI not available (MS Q&A 1696159, ctrlaltnod 2026-03).
- Network: netsh int tcp global + Tcpip\Parameters registry (gist pyyupsk, gist asheroto, SpeedGuide "Windows 10,11 TCP/IP Tweaks", MS "TCP/IP performance known issues", tenforums AutoTuningLevel), Get-NetTCPSetting/Set-NetTCPSetting.
- Games: Steam registry path + libraryfolders.vdf/appmanifest.acf (SO 58388230, SO 78531838, DeepWiki SteamShutdown), Epic manifests %ProgramData%\Epic\...\Manifests (r/gamedev 1agkenr), Battle.net agent.db, Riot registry, Xbox aggregated library (The Verge 2025-06), SteamLibrarian process-detection approach.
- Event log / crash: MS Learn "Using WER", "Crash Dump Analysis" (MiniDumpWriteDump), Event IDs 1000/1001 (SoftwareVerify 2020), windows-docs-rs Win32::System::EventLog (EvtQuery/EvtRender/EvtSubscribe), WER ReportArchive paths, rust-minidump/symbolic ecosystem.
- Benchmark: PresentMon v2.3.1 (presentmon.com, GameTechDev/PresentMon, issue #219 percentile semantics, TechSpot PresentMon guide, FrameView docs: MsBetweenDisplayChange vs MsBetweenPresents).
- SQLite: tauri-plugin-sql 2.4.0 docs, tauri-plugin-rusqlite2 (fork), rusqlite 0.40.
- Windows 10/11: microsoft.com/windows/end-of-support (Win10 EOL 2025-10-14), Windows Central (2025-08-14; Win11 ≥ Win10 with defaults fixed), IQON (2026-02), HD Opti (2026-03/06), guru3D EOL thread (2025-10).
- Bloatware/AppX: PDQ "How to remove bloatware from Windows 11" (allowlist practice), samuelkranec/windows-bloatware-remover, Win11Debloat (sujirou), UMATechnology 7-quick-ways (2026-05), DISM Remove-ProvisionedAppxPackage + 25H2 "Remove default Microsoft Store packages" GPO (MS Q&A 5619791, mundobytes 2025-09), DISM startcomponentcleanup (MS Learn), itechguides (2026-08).
- Windows Search: NinjaOne "Enable/Disable Search Indexing" (2026-04; backoff settings, index DB relocation, gaming-PC rationale), tecnobits SearchIndexer.exe (2026-05; "delete C:\ProgramData\Microsoft\Search\Data to regenerate" — we never auto-delete), itechguides "Turn off indexing — should you?" (2026-08; Enhanced→Classic is safer), softwareok WSearch service, windowsdigitals 30+ services (2024-04).
- DNS: pinggy "Best DNS for Gaming 2026" (resolver table incl. Control D, NextDNS, Mullvad, DNS4EU; GRC DNS Benchmark, NAMEinator, dnsperf; "DNS does not lower in-game ping"), bufferspeed (2026-07; DoH/DoT adds 5-15 ms, router port-53 override caveat), theinfobits 9-resolver ranking (2026-07), gamingpcguru (2026-05).
- Monitoring: MS Learn Performance Counters Functions (PDH API list; PdhAddEnglishCounterW, PdhExpandWildCardPath, rate counters need 2 samples), simpleobservability "Windows Monitoring Guide 2026" (PDH vs WMI overhead table, thresholds: disk <10 ms/>50 ms, Available MBytes <5-10%, output queue length = 0), Splunk perfmon input docs (useWinApiProcStats / GetProcessTimes multicore practice, localized counter names), sysinfo windows/cpu.rs (uses PdhAddEnglishCounterW), ratijas/windows-rust-counters.
- Process management: Ahmad-Bin-Rashid/Windows-Task-Manager-CLI (full Win32 API table: Toolhelp32, GetProcessTimes, NtSuspendProcess, GetExtendedTcpTable, priority class values 4/6/8/10/13/24), MS Learn CPU Sets (soft affinity, hybrid-CPU friendly), MS Learn OpenProcess (PROCESS_QUERY_LIMITED_INFORMATION, SeDebugPrivilege), middaysan/cpu-affinity-tool.
- GPU vendor APIs: NVAPI SDK (docs.nvidia.com/nvapi, release 590 Feb 2026, GitHub NVIDIA/nvapi, DRS Programming Guide), AMD ADLX SDK v1.5 (Apr 2026; gpuopen.com/adlx, GPUOpen-LibrariesAndSDKs/ADLX, adlx-rs bindings, IADLXGPUMetrics GPUFanDuty), Intel IGCL (intel/drivers.gpu.control-library; Control API + hardware monitoring/telemetry), Intel Unified Telemetry (beta, Lunar Lake+), Intel "Supported APIs for Intel Graphics" (000005524, Arc DX12/OGL4.6/Vulkan 1.3-1.4).

---

*End of plan. Every phase above is implementation-ready: the specific API, crate, registry key, GUID, or command to use is stated, with the safety/rollback path defined for each.*
