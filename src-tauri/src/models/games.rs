//! Game Profile System models (Phase 9): detected games, the saved library,
//! and per-game optimization profiles.

use serde::{Deserialize, Serialize};

use crate::models::process::PriorityChange;

/// A game discovered by a launcher scan, before it is added to the library.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedGame {
    pub name: String,
    /// steam | epic | riot | battlenet | manual
    pub launcher: String,
    /// Launcher-specific id (Steam appid, etc.).
    pub app_id: Option<String>,
    pub install_path: String,
    /// Full path to the game executable, or empty when unknown.
    pub executable: String,
}

/// A game in the Optix library, annotated with live running state.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: i64,
    pub name: String,
    pub launcher: String,
    pub app_id: Option<String>,
    pub install_path: String,
    pub executable: String,
    /// Basename of `executable` (lowercased), used to match running processes.
    pub exe_name: String,
    pub last_played: Option<i64>,
    pub detected_at: Option<i64>,
    /// Whether the game's executable is currently running.
    pub running: bool,
    /// PIDs of the running game processes.
    pub pids: Vec<u32>,
    /// Whether the game-mode watcher has boosted this game right now.
    pub boosted: bool,
}

/// Per-game optimization profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameProfile {
    pub game_id: i64,
    /// normal | above_normal | high. REALTIME is never accepted.
    pub cpu_priority: String,
    /// Optional hex bitmask of allowed logical processors.
    pub affinity_mask: Option<String>,
    /// none | balanced_gaming | competitive_gaming | maximum_performance
    pub power_profile: String,
    /// none | dns | tcp_experimental (manual actions, not auto-applied).
    pub network_profile: String,
    /// Lower SAFE background processes while the game runs.
    pub cleanup_bg: bool,
    /// Optional NVIDIA DRS per-game profile: `Some("nvidia")` applies an
    /// `Optix: <game>` driver-settings profile on apply (see `win::nvapi`).
    pub gpu_profile: Option<String>,
    /// Whether the game-mode watcher auto-applies this profile on launch.
    pub enabled: bool,
}

/// A CPU-affinity change applied to a game process.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AffinityChange {
    pub pid: u32,
    pub name: String,
    /// Previous mask (None when it could not be read).
    pub from: Option<u64>,
    pub to: u64,
}

/// Result of applying a game profile.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameProfileApplyResult {
    /// Set when a power profile was applied (snapshot-first, reversible).
    pub snapshot_id: Option<String>,
    /// Name of the applied power profile, if any.
    pub power_applied: Option<String>,
    pub boosted: Vec<PriorityChange>,
    pub lowered: Vec<PriorityChange>,
    pub affinity_applied: Vec<AffinityChange>,
    /// Name of the NVIDIA DRS profile created for this game, when one was
    /// applied (NVIDIA hardware + `gpu_profile: nvidia` only).
    pub gpu_profile: Option<String>,
}
