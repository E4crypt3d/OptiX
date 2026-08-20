use serde_json::Value;
use tauri::State;

use crate::db::sqlite::Database;
use crate::engine::{rollback, snapshot};
use crate::error::{OptixError, Result};
use crate::models::snapshot::{ChangeRecord, Snapshot};

/// Create a snapshot of the current system state. Runs the scan on a blocking
/// thread (it performs WMI queries on Windows).
#[tauri::command]
pub async fn create_snapshot(
    db: State<'_, Database>,
    name: String,
    reason: Option<String>,
) -> Result<Snapshot> {
    let system = tauri::async_runtime::spawn_blocking(crate::commands::system::scan_system_blocking)
        .await
        .map_err(|e| OptixError::Other(e.to_string()))??;
    snapshot::create(db.inner(), &name, reason.as_deref(), &system)
}

#[tauri::command]
pub fn list_snapshots(db: State<'_, Database>) -> Result<Vec<Snapshot>> {
    db.list_snapshots()
}

#[tauri::command]
pub fn list_changes(db: State<'_, Database>, snapshot_id: String) -> Result<Vec<ChangeRecord>> {
    db.list_changes(&snapshot_id)
}

#[tauri::command]
pub fn delete_snapshot(db: State<'_, Database>, id: String) -> Result<()> {
    snapshot::delete(db.inner(), &id)
}

#[tauri::command]
pub fn restore_snapshot(db: State<'_, Database>, id: String) -> Result<usize> {
    rollback::restore(db.inner(), &id)
}

#[tauri::command]
pub fn diff_snapshots(db: State<'_, Database>, a: String, b: String) -> Result<Value> {
    rollback::diff(db.inner(), &a, &b)
}

/// Create a System Restore point (`SRSetRestorePoint`) as an extra safety net
/// before destructive operations. Fails cleanly when System Protection is
/// disabled. Returns the restore-point sequence number when one was created.
#[tauri::command]
pub fn create_system_restore_point(description: String) -> Result<Option<u32>> {
    crate::win::restorepoint::create_restore_point(&description)
}
