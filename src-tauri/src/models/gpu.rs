//! GPU management models (Phase 8): gaming toggles, shader cache inventory,
//! and adapter summary.

use serde::Serialize;

/// A gaming-related registry toggle (HAGS, GameDVR, VBS, Game Mode, MPO).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamingToggle {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Current "on" state (default interpretation when the value is absent).
    pub enabled: bool,
    /// False when the registry value is absent (driver/OS default applies).
    pub known: bool,
    pub impact_note: String,
    /// "low" | "medium" | "high".
    pub risk: String,
    pub requires_restart: bool,
}

/// A GPU shader cache directory and its measured size.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShaderCache {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub file_count: u64,
    pub description: String,
}

/// Result of applying a gaming toggle.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuToggleResult {
    pub snapshot_id: String,
    pub changes: usize,
}

/// Result of clearing shader caches.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheClearResult {
    pub snapshot_id: String,
    pub freed_bytes: u64,
    pub files_removed: u64,
}

/// A detected display adapter with live telemetry.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuAdapter {
    pub name: String,
    pub vendor: String,
    pub driver_version: String,
    /// Total dedicated VRAM in bytes (0 if unknown).
    pub memory_bytes: u64,
    /// Current VRAM usage in bytes (None if unsupported).
    pub memory_used_bytes: Option<u64>,
    /// GPU core temperature in Celsius (None if unsupported).
    pub temperature_celsius: Option<f32>,
    /// GPU core utilization percent 0–100 (None if unsupported).
    pub usage_percent: Option<f32>,
}

/// AMD shader cache mode (registry `UMD\ShaderCache`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AmdShaderCache {
    pub adapter: String,
    /// "always_on" | "optimized" | "unknown".
    pub mode: String,
}
