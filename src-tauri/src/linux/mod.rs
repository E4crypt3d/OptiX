//! Linux-only integrations, mirroring the `win/` module's shape: every
//! function is `#[cfg(target_os = "linux")]`-gated with a fallback so the
//! crate still compiles on other platforms.
//!
//! - Services: systemd units (system + user scope) via `systemctl`.
//! - Startup apps: XDG autostart `.desktop` entries.
//! - Scheduled tasks: systemd timers + cron.

pub mod services;
pub mod startup;
pub mod tasks;
