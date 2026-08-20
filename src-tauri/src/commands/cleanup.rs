use tauri::State;

use crate::db::sqlite::Database;
use crate::engine::{cleanup, rollback, snapshot};
use crate::error::{OptixError, Result};
use crate::models::cleanup::{CategoryResult, CleanupCategory, CleanupResult};
use crate::models::snapshot::ChangeRecord;

/// Scan cleanup categories (directory walking runs on a blocking thread).
#[tauri::command]
pub async fn scan_cleanup() -> Result<Vec<CleanupCategory>> {
    tauri::async_runtime::spawn_blocking(cleanup::scan)
        .await
        .map_err(|e| OptixError::Other(e.to_string()))
}

/// Run cleanup: snapshot-first, then delete the selected categories' contents.
#[tauri::command]
pub async fn run_cleanup(db: State<'_, Database>, mut ids: Vec<String>) -> Result<CleanupResult> {
    ids.sort();
    ids.dedup();
    cleanup::validate_ids(&ids)?;

    let snapshot = snapshot::create_lightweight(
        db.inner(),
        "Cleanup",
        Some(&format!("cleanup: {}", ids.join(", "))),
    )?;

    // Extra safety net before destructive deletion: a System Restore point,
    // best-effort (fails cleanly when System Protection is disabled). Failures
    // are logged, never hidden — cleanup still proceeds.
    if let Err(e) = crate::win::restorepoint::create_restore_point("Optix cleanup") {
        crate::logging::warn(&format!("system restore point skipped: {e}"));
    }

    let outcomes = tauri::async_runtime::spawn_blocking(move || cleanup::delete_categories(&ids))
        .await
        .map_err(|e| OptixError::Other(e.to_string()))??;

    let mut result = CleanupResult {
        snapshot_id: snapshot.id.clone(),
        freed_bytes: 0,
        files_removed: 0,
        files_skipped: 0,
        categories: Vec::new(),
    };

    for o in outcomes {
        let change = ChangeRecord {
            id: None,
            snapshot_id: String::new(),
            domain: "file".into(),
            location: format!("cleanup:{}", o.id),
            kind: "delete".into(),
            old_value: Some(o.before_bytes.to_string()),
            new_value: Some(o.before_bytes.saturating_sub(o.freed_bytes).to_string()),
            old_json: None,
            new_json: None,
            applied_at_ms: None,
            verified: true,
            rolled_back: false,
        };
        rollback::record_change(db.inner(), &result.snapshot_id, change)?;

        result.freed_bytes = result.freed_bytes.saturating_add(o.freed_bytes);
        result.files_removed = result.files_removed.saturating_add(o.files_removed);
        result.files_skipped = result.files_skipped.saturating_add(o.files_skipped);
        result.categories.push(CategoryResult {
            id: o.id,
            before_bytes: o.before_bytes,
            freed_bytes: o.freed_bytes,
            files_removed: o.files_removed,
            files_skipped: o.files_skipped,
        });
    }

    Ok(result)
}

/// Run DISM WinSxS component cleanup (`/startcomponentcleanup` — never
/// `resetbase`). Admin required; streamed output returned for the UI.
#[tauri::command]
pub async fn dism_component_cleanup(db: State<'_, Database>) -> Result<String> {
    #[cfg(windows)]
    {
        return dism_component_cleanup_windows(db).await;
    }
    #[cfg(not(windows))]
    {
        let _ = db;
        Err(crate::error::OptixError::UnsupportedPlatform(
            "DISM component cleanup".into(),
        ))
    }
}

#[cfg(windows)]
async fn dism_component_cleanup_windows(db: State<'_, Database>) -> Result<String> {
    let snapshot = snapshot::create_lightweight(
        db.inner(),
        "DISM component cleanup",
        Some("WinSxS component store cleanup"),
    )?;
    rollback::record_change(
        db.inner(),
        &snapshot.id,
        ChangeRecord {
            id: None,
            snapshot_id: String::new(),
            domain: "file".into(),
            location: "winsxs:component_cleanup".into(),
            kind: "replace".into(),
            old_value: None,
            new_value: None,
            old_json: None,
            new_json: None,
            applied_at_ms: None,
            verified: true,
            rolled_back: false,
        },
    )?;

    if let Err(e) = crate::win::restorepoint::create_restore_point("Optix DISM component cleanup")
    {
        crate::logging::warn(&format!("system restore point skipped: {e}"));
    }
    tauri::async_runtime::spawn_blocking(crate::win::cleanup::run_dism_component_cleanup)
        .await
        .map_err(|e| OptixError::Other(e.to_string()))?
}
