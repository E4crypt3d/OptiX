//! Windows-only integrations. Every function is `#[cfg(windows)]`-gated with a
//! non-Windows fallback so the crate still compiles and runs on Linux during
//! development. Windows-specific crates (`winreg`, `wmi`, `windows-version`)
//! are declared under `[target.'cfg(windows)'.dependencies]` in `Cargo.toml`.

pub mod elevation;
pub mod enrich;
pub mod games;
pub mod gpu;
pub mod hardware;
pub mod network;
pub mod nic;
pub mod os;
pub mod power;
pub mod process;
pub mod registry;
pub mod services;
pub mod startup;
pub mod wmi;
