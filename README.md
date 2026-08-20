# Optix

Windows gaming performance optimization and system recovery utility.

> **Optimize your PC without fear. Every change is tracked. Every change can be undone.**

Optix is a system management platform, not a fake "FPS booster". Every
modification follows a fixed pipeline:

**Detect → Snapshot → Apply → Verify → Record → Rollback.**

---

## What Optix does

Optix fixes the Windows defaults that slow games down, cleans up what's safe to
remove, and helps you measure the difference — then lets you undo any of it.

A few honest ground rules up front:

- **Nothing is permanent.** Every change is snapshot-first and reversible from
  the Rollback Center.
- **Nothing is deleted automatically.** Cleanup and bloatware removal always
  require your explicit confirmation.
- **Nothing is "fake".** Optix shows measured evidence (benchmarks, telemetry,
  crash reports) instead of promising free FPS.
- **Security stays on unless you choose otherwise.** Memory Integrity / VBS and
  similar toggles are opt-in, clearly labeled, and reversible.

## Features

| Module | What it does |
|--------|--------------|
| **Dashboard** | Live CPU, memory, and network telemetry with trend charts. |
| **System Scanner** | Full hardware + software inventory: CPU, GPU, RAM, disks (type + health), displays, OS build, running processes, and startup apps. |
| **Snapshots** | Capture the state of every area Optix touches, before you change anything. |
| **Rollback Center** | Inspect every tracked change, restore snapshots in reverse order, and diff two snapshots. |
| **Cleanup** | Safely reclaim space from temp files, browser caches, GPU shader caches, crash dumps, and old logs — with a hard deny-list so it never touches protected system paths. |
| **Bloatware** | Review preinstalled Store apps, with a protected allowlist (never flags core system or Xbox packages) and removal of provisioned copies so they don't reinstall. |
| **Processes & RAM** | See every running process with a REQUIRED / SAFE / UNKNOWN classification, change priority (never REALTIME), and apply a gaming mode that boosts your game and lowers background apps. |
| **Power** | Apply Optix power profiles (Balanced / Competitive / Maximum) cloned from built-ins, and disable network-adapter power saving — all reversible. |
| **Startup & Services** | Review what runs at boot and in the background, with a hard never-flag list, plus a dedicated Windows Search toggle. |
| **Network** | Benchmark public DNS resolvers (latency + packet loss) and apply the fastest one; inspect TCP/IP parameters. |
| **GPU** | Review risk-tiered gaming toggles (HAGS, Game DVR, Memory Integrity/VBS, Game Mode, MPO), clear shader caches, and set the AMD shader-cache mode. |
| **Game Profiles** | Auto-detect installed games (Steam, Epic, Riot, Battle.net), add games manually, and set per-game CPU priority, affinity, and power profile — applied automatically on launch. |
| **Benchmarks** | Capture FPS with PresentMon (avg FPS, 1%/0.1% lows, frame-time chart) or run a system-stress benchmark, and compare runs before/after an optimization. |
| **Crash Reports** | Scan the Windows Event Log, WER reports, and minidumps; classify crashes and export a `CrashReport.zip` for support. |
| **Diagnostics** | Rule-based analysis with confidence scores and evidence — advisory only, never changes anything. |

## Getting started

### Requirements

- **Windows 11** (primary) or **Windows 10** (secondary — both share the same
  supported APIs).
- Administrator privileges. Optix relaunches itself through UAC when it isn't
  already elevated, because registry, services, power schemes, and process
  controls all require admin.

### Install & run (Windows)

1. Download the latest installer from the
   [Releases](../../releases) page.
2. Run the **NSIS** installer (`Optix_<version>_x64-setup.exe`).
3. Launch Optix. On first run it asks for elevation — approve it.
4. Before changing anything, create a snapshot from the **Snapshots** page.

> **Tip:** Optix stores its database and snapshots under
> `C:\ProgramData\Optix`. If you ever want to start fresh, that's the directory
> to remove (do this while Optix is closed).

### Linux support (Pop!_OS / Ubuntu)

Linux builds are fully supported as a development target and are packaged as
`.deb` and AppImage from CI. The scanner, snapshots, cleanup, DNS benchmark,
stress benchmark, and telemetry all work on Linux; registry-, service-, power-,
GPU-driver- and PresentMon-based features are Windows-only and are
gracefully hidden or report "Windows-only" instead of failing.

**Run from source** on Pop!_OS 22.04/24.04 or Ubuntu 22.04/24.04:

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential libxdo-dev \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev
npm install
npm run tauri dev
```

**Install a packaged build** from the [Releases](../../releases) page:

- `optix_<version>_amd64.deb` → `sudo apt install ./optix_<version>_amd64.deb`
- `optix_<version>_amd64.AppImage` → `chmod +x`, then run it

On Linux, Optix stores its database and snapshots under
`~/.local/share/optix`.

## Honest expectations

Not every tweak helps every game. Roughly, by measured impact:

1. **Disable Memory Integrity / VBS** — the single biggest lever on Windows 11
   (up to 5–15% in some titles), but a real security trade-off. Opt-in only.
2. **Disable Game DVR / Game Bar background recording** — frees RAM and cuts
   input latency.
3. **Power plan** — High/Ultimate Performance or an Optix profile.
4. **GPU driver settings + shader cache** — reduces stutter, not raw FPS.
5. **Process priority/affinity during gaming** — a modest, real gain.
6. **DNS selection** — helps launchers, logins, and matchmaking, but **not**
   in-game ping once a connection is established.
7. **TCP/IP tweaks** — mostly placebo on modern Windows; marked experimental.

Use the **Benchmarks** page to measure before/after on the same scene rather
than trusting the label.

---

## Contributing

Optix is built as a Tauri app: a **Rust** backend with **React + TypeScript**
frontend. Contributions are welcome — the section below gets you from clone to
pull request.

### Stack

- **Frontend**: React 19 + TypeScript, Tailwind CSS v4, Vite, Recharts
- **Backend**: Rust, Tauri v2, `sysinfo` (cross-platform scanner), `rusqlite`
  (bundled SQLite), `windows-sys` + `winreg` (Windows integration), `zip`
  (crash-report bundles)
- **Database**: SQLite at `C:\ProgramData\Optix\optix.db`

### Prerequisites

- [Node.js](https://nodejs.org) 20+ and npm
- [Rust](https://rustup.rs) stable (MSRV 1.85)
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
for frontend and engine work.

### Project structure

```
src/                     React frontend (Vite + Tailwind)
  components/            one component per page (Dashboard, Cleanup, …)
  lib/                   api.ts (typed invoke wrappers), types.ts, format helpers
src-tauri/
  src/
    main.rs              elevation bootstrap + entry point
    lib.rs               module tree + command registration
    error.rs             OptixError (thiserror, serialized to the frontend)
    commands/            Tauri commands — one file per feature area
    engine/              platform-independent logic (cleanup, rollback, …)
    models/              serde structs shared between commands and the frontend
    db/                  SQLite schema + migrations (sqlite.rs)
    win/                 #[cfg(windows)]-only integrations (registry, services, …)
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

### CI

GitHub Actions runs backend tests on Linux, builds `.deb` + AppImage installers
on `ubuntu-latest`, and builds NSIS + MSI installers on `windows-latest`.

### Roadmap / status

Completed phases: Foundation, System Scanner, Power, Startup & Services,
Network, GPU, Game Profiles, Benchmark, Crash Recovery, Diagnostics.

In-progress (mostly done, with noted gaps):

- **Snapshot & Recovery** — snapshot/rollback/diff done; remaining: System
  Restore point.
- **Cleanup** — safe-category scanner + bloatware done; remaining:
  SoftwareDistribution + Recycle Bin categories, DISM component cleanup.
- **Process & RAM** — classification, priority, gaming mode, affinity done;
  remaining: per-process GPU (PDH).

Known follow-ups across the codebase: ICMP ping/jitter, TCP-tweak apply +
one-click reset, scheduled-task enumeration + publisher/signature
verification, NVIDIA DRS per-game profiles, bundling the PresentMon binary, and
a live crash-watch subscription.

### Contributing guidelines

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
