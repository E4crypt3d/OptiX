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
pub async fn run_cleanup(db: State<'_, Database>, ids: Vec<String>) -> Result<CleanupResult> {
    let snapshot = snapshot::create_lightweight(
        db.inner(),
        "Cleanup",
        Some(&format!("cleanup: {}", ids.join(", "))),
    )?;

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

        result.freed_bytes += o.freed_bytes;
        result.files_removed += o.files_removed;
        result.files_skipped += o.files_skipped;
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
