# Optix

**Optimize your PC without fear. Every change is tracked. Every change can be undone.**

Optix is a Windows gaming-performance and system-recovery utility. It fixes the
Windows defaults that slow games down, cleans up what's safe to remove, helps
you measure the difference — and lets you undo any of it.

Every modification follows the same pipeline:

**Detect → Snapshot → Apply → Verify → Record → Rollback.**

---

## What you can do with Optix

| Module | What it does |
|--------|--------------|
| **Dashboard** | Live CPU, memory, and network telemetry with trend charts. |
| **System Scanner** | Full hardware + software inventory: CPU, GPU, RAM, disks (type + health), displays, OS build, running processes, and startup apps. |
| **Snapshots** | Capture the state of every area Optix touches before you change anything — plus optional Windows **System Restore points** for an extra safety net. |
| **Rollback Center** | Inspect every tracked change, restore snapshots in reverse order, and diff two snapshots. |
| **Cleanup** | Reclaim space from temp files, browser caches, GPU shader caches, crash dumps, old logs, Windows Update leftovers, and the Recycle Bin — with a hard deny-list so protected system paths are never touched. Includes Microsoft-sanctioned DISM WinSxS component cleanup. |
| **Bloatware** | Review preinstalled Store apps with a protected allowlist (never flags core system or Xbox packages), and remove provisioned copies so they don't reinstall. |
| **Processes & RAM** | See every running process with a REQUIRED / SAFE / UNKNOWN classification, per-process **GPU usage**, priority control (never REALTIME), and a gaming mode that boosts your game and lowers background apps. |
| **Power** | Apply Optix power profiles (Balanced / Competitive / Maximum) cloned from the built-in schemes, and disable network-adapter power saving — all reversible. |
| **Startup & Services** | Review what runs at boot and in the background, with a hard never-flag list, a dedicated Windows Search toggle, and **scheduled-task** enumeration with signature verification. |
| **Network** | Benchmark DNS resolvers (latency + packet loss) and apply the fastest one; **ICMP ping test** (RTT + jitter); experimental **TCP/IP tweaks** with one-click apply and revert. |
| **GPU** | Risk-tiered gaming toggles (HAGS, Game DVR, Memory Integrity/VBS, Game Mode, MPO), shader-cache management, AMD shader-cache mode, and **NVIDIA per-game driver profiles** (DRS). |
| **Game Profiles** | Auto-detect installed games (Steam, Epic, Riot, Battle.net) or add them manually, and set per-game CPU priority, affinity, power profile, and an optional NVIDIA profile — applied automatically on launch. |
| **Benchmarks** | Capture FPS with the bundled PresentMon (avg FPS, 1% / 0.1% lows, frame-time chart) or run a system-stress benchmark, and compare runs before/after an optimization. |
| **Crash Reports** | Scan the Windows Event Log, WER reports, and minidumps; classify crashes, **watch for new crashes live**, and export a `CrashReport.zip` for support. |
| **Diagnostics** | Rule-based analysis with confidence scores and evidence — advisory only, never changes anything. |

## Honest expectations

Optix is not a "free FPS" booster. Not every tweak helps every game. Roughly,
by measured impact:

1. **Disable Memory Integrity / VBS** — the single biggest lever on Windows 11
   (up to 5–15% in some titles), but a real security trade-off. Opt-in only.
2. **Disable Game DVR / Game Bar background recording** — frees RAM and cuts
   input latency.
3. **Power plan** — High/Ultimate Performance or an Optix profile.
4. **GPU driver settings + shader cache** — reduces stutter, not raw FPS.
5. **Process priority/affinity during gaming** — a modest, real gain.
6. **DNS selection** — helps launchers, logins, and matchmaking, but **not**
   in-game ping once a connection is established.
7. **TCP/IP tweaks** — mostly placebo on modern Windows; clearly marked
   experimental.

Use the **Benchmarks** page to measure before/after on the same scene rather
than trusting the label.

## Safety model

- **Nothing is permanent.** Every change is snapshot-first and reversible from
  the Rollback Center.
- **Nothing is deleted automatically.** Cleanup and bloatware removal always
  require your explicit confirmation.
- **Security stays on unless you choose otherwise.** Memory Integrity / VBS and
  similar toggles are opt-in, clearly labeled, and reversible.
- **Problems are visible.** Every error is written to `logs.txt` next to the
  installed Optix executable (find the exact path in **Settings**) and to the
  console, so nothing fails silently.

---

## Getting started

### Requirements

- **Windows 11** (primary) or **Windows 10** (secondary).
- **Administrator privileges.** Optix relaunches itself through UAC when it
  isn't already elevated, because registry, services, power schemes, and
  process controls all require admin.

### Install

1. Download the latest installer from the [Releases](../../releases) page.
2. Run the **NSIS** installer (`Optix_<version>_x64-setup.exe`).
3. Launch Optix. On first run it asks for elevation — approve it.
4. Before changing anything, create a snapshot from the **Snapshots** page.

> **Tip:** Optix stores its database, snapshots, and logs under
> `C:\ProgramData\Optix`. To start fresh, remove that directory while Optix is
> closed.

---

## For developers

Optix is a Tauri app: a **Rust** backend with a **React + TypeScript**
frontend.

### Stack

- **Frontend**: React 19 + TypeScript, Tailwind CSS v4, Vite, Recharts
- **Backend**: Rust, Tauri v2, `sysinfo` (cross-platform scanner), `rusqlite`
  (bundled SQLite), `windows-sys` + `winreg` (Windows integration), `zip`
  (crash-report bundles)
- **Database**: SQLite at `C:\ProgramData\Optix\optix.db`

### Prerequisites

- [Node.js](https://nodejs.org) 20+ and npm
- [Rust](https://rustup.rs) stable (MSRV 1.87)
- On Linux: `webkit2gtk` and the system dependencies Tauri requires (see the
  [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/))

### Setup

```bash
git clone <this-repo>
cd Optix
npm install
npm run tauri dev
```

On a Windows host, `npm run tauri dev` runs the full app (Windows integration
included). On Linux it runs the UI with Windows-only modules stubbed — useful
for frontend and engine work. Linux builds are packaged as `.deb` and AppImage
from CI.

### Project structure

```
src/                     React frontend (Vite + Tailwind)
  components/            one component per page (Dashboard, Cleanup, …)
  lib/                   api.ts (typed invoke wrappers), types.ts, format helpers
src-tauri/
  src/
    main.rs              elevation bootstrap + entry point
    lib.rs               module tree + command registration + startup services
    logging.rs           console + logs.txt logging (errors are never swallowed)
    error.rs             OptixError (thiserror, serialized to the frontend)
    commands/            Tauri commands — one file per feature area
    engine/              platform-independent logic (cleanup, rollback, …)
    models/              serde structs shared between commands and the frontend
    db/                  SQLite schema + migrations (sqlite.rs)
    win/                 #[cfg(windows)]-only integrations (registry, services,
                         PDH, NVAPI, ping, restore points, …)
```

### How it fits together

- The frontend calls typed wrappers in `src/lib/api.ts`, which `invoke` Tauri
  commands registered in `src-tauri/src/lib.rs`.
- Commands live in `src-tauri/src/commands/`, orchestrate work in
  `src-tauri/src/engine/`, and persist via `src-tauri/src/db/`.
- Every struct returned to the frontend is in `src-tauri/src/models/` and uses
  `#[serde(rename_all = "camelCase")]`; mirror it in `src/lib/types.ts`.
- Windows-only code is gated behind `#[cfg(windows)]` with a non-Windows stub
  so the crate still compiles on Linux.
- Errors are logged through `src-tauri/src/logging.rs` (console + `logs.txt`)
  — never silently dropped.

### Tests

```bash
# Backend (engine + db logic, runs on Linux)
cd src-tauri && cargo test

# Windows-target compile check (validates winreg/windows-sys FFI)
cd src-tauri && cargo check --target x86_64-pc-windows-gnu

# Frontend type-check + production build
npm run build
```

### Building the Windows installer

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

The PresentMon benchmark binary is fetched into `src-tauri/resources/` by
`scripts/fetch-presentmon.ps1` and bundled automatically; CI runs the fetch
before packaging.

### CI

GitHub Actions runs backend tests on Linux, builds `.deb` + AppImage installers
on `ubuntu-latest`, and builds NSIS + MSI installers on `windows-latest`.

### Contributing

- Use **conventional commits**: `feat:`, `fix:`, `refactor:`, `chore:`,
  `docs:`, etc.
- Keep changes focused and atomic; preserve existing behavior unless the change
  is intentional.
- Match existing patterns — new features follow
  `models/ → engine/ → commands/ → registration → frontend types + api + page`.
- Add unit tests for new pure logic (classifiers, parsers, math) and run the
  checks above before opening a PR.
- Never introduce irreversible changes, auto-delete, REALTIME priority, or
  unprotected security-feature changes — the safety model is the point.

## License

See the [LICENSE](LICENSE) file.
