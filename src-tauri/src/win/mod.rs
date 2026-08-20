//! Windows-only integrations. Every function is `#[cfg(windows)]`-gated with a
//! non-Windows fallback so the crate still compiles and runs on Linux during
//! development. Windows-specific crates (`winreg`, `wmi`, `windows-version`)
//! are declared under `[target.'cfg(windows)'.dependencies]` in `Cargo.toml`.

pub mod appx;
pub mod cleanup;
pub mod crash;
pub mod elevation;
pub mod enrich;
pub mod games;
pub mod gpu;
pub mod hardware;
pub mod network;
pub mod nic;
pub mod nvapi;
pub mod os;
pub mod pdh;
pub mod ping;
pub mod power;
pub mod presentmon;
pub mod process;
pub mod registry;
pub mod restorepoint;
pub mod services;
pub mod signature;
pub mod startup;
pub mod tasks;
pub mod wmi;
