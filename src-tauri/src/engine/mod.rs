pub mod benchmark;
pub mod bloatware;
pub mod cleanup;
pub mod crash;
pub mod diagnostics;
pub mod game_watcher;
pub mod games;
pub mod gpu;
pub mod network;
pub mod optimizer;
pub mod power;
pub mod processes;
pub mod rollback;
pub mod services;
pub mod snapshot;

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
