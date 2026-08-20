use tauri::State;

use crate::db::sqlite::Database;
use crate::engine::{gpu, rollback, snapshot};
use crate::error::{OptixError, Result};
use crate::models::gpu::{
    AmdShaderCache, CacheClearResult, GamingToggle, GpuAdapter, GpuToggleResult, ShaderCache,
};
use crate::models::snapshot::ChangeRecord;

/// Detected display adapters (WMI-backed; runs off the main thread).
#[tauri::command]
pub async fn list_gpu_adapters() -> Result<Vec<GpuAdapter>> {
    tauri::async_runtime::spawn_blocking(gpu::list_adapters)
        .await
        .map_err(|e| OptixError::Other(e.to_string()))
}

/// Current gaming toggles (HAGS, GameDVR, VBS, Game Mode, MPO).
#[tauri::command]
pub fn list_gpu_toggles() -> Vec<GamingToggle> {
    gpu::list_toggles()
}

/// Apply a gaming toggle (snapshot-first, reversible).
#[tauri::command]
pub fn set_gpu_toggle(db: State<'_, Database>, id: String, enabled: bool) -> Result<GpuToggleResult> {
    gpu::set_toggle(db.inner(), &id, enabled)
}

/// Shader cache inventory (sizes computed on a blocking thread).
#[tauri::command]
pub async fn scan_shader_caches() -> Result<Vec<ShaderCache>> {
    tauri::async_runtime::spawn_blocking(gpu::scan_caches)
        .await
        .map_err(|e| OptixError::Other(e.to_string()))
}

/// Clear selected shader caches (snapshot-first, deletion on a blocking thread).
#[tauri::command]
pub async fn clear_shader_caches(
    db: State<'_, Database>,
    ids: Vec<String>,
) -> Result<CacheClearResult> {
    let snapshot = snapshot::create_lightweight(
        db.inner(),
        "Shader cache",
        Some(&format!("clear shader caches: {}", ids.join(", "))),
    )?;

    let outcomes = tauri::async_runtime::spawn_blocking(move || gpu::clear_cache_dirs(&ids))
        .await
        .map_err(|e| OptixError::Other(e.to_string()))?;

    let mut result = CacheClearResult {
        snapshot_id: snapshot.id.clone(),
        freed_bytes: 0,
        files_removed: 0,
    };
    for o in outcomes {
        rollback::record_change(
            db.inner(),
            &result.snapshot_id,
            ChangeRecord {
                id: None,
                snapshot_id: String::new(),
                domain: "file".to_string(),
                location: format!("shader_cache:{}", o.id),
                kind: "delete".to_string(),
                old_value: Some(o.before_bytes.to_string()),
                new_value: Some(o.before_bytes.saturating_sub(o.freed_bytes).to_string()),
                old_json: None,
                new_json: None,
                applied_at_ms: None,
                verified: true,
                rolled_back: false,
            },
        )?;
        result.freed_bytes += o.freed_bytes;
        result.files_removed += o.files_removed;
    }
    Ok(result)
}

/// Current AMD shader cache mode.
#[tauri::command]
pub fn get_amd_shader_cache() -> AmdShaderCache {
    gpu::amd_shader_cache()
}

/// Set the AMD shader cache mode (always_on = true, else optimized),
/// snapshot-first and reversible.
#[tauri::command]
pub fn set_amd_shader_cache(
    db: State<'_, Database>,
    always_on: bool,
) -> Result<AmdShaderCache> {
    gpu::set_amd_shader_cache(db.inner(), always_on)
}
