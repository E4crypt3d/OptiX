use tauri::State;

use crate::db::sqlite::Database;
use crate::engine::{bloatware, rollback, snapshot};
use crate::error::{OptixError, Result};
use crate::models::bloatware::{AppxPackage, AppxRemovalFailure, BloatwareRemoveResult};
use crate::models::snapshot::ChangeRecord;

/// Scan installed AppX packages and classify them for removal.
#[tauri::command]
pub async fn scan_bloatware() -> Result<Vec<AppxPackage>> {
    tauri::async_runtime::spawn_blocking(|| bloatware::scan().map_err(OptixError::Other))
        .await
        .map_err(|e| OptixError::Other(e.to_string()))?
}

/// Remove the selected packages (snapshot-first). Provisioned copies are
/// removed first so they do not reinstall for new users.
#[tauri::command]
pub async fn remove_bloatware(
    db: State<'_, Database>,
    mut full_names: Vec<String>,
) -> Result<BloatwareRemoveResult> {
    full_names.sort();
    full_names.dedup();
    bloatware::validate_removal(&full_names).map_err(OptixError::Other)?;

    let snapshot = snapshot::create_lightweight(
        db.inner(),
        "Bloatware",
        Some(&format!("bloatware: {}", full_names.join(", "))),
    )?;

    let outcomes = tauri::async_runtime::spawn_blocking(move || bloatware::remove(&full_names))
        .await
        .map_err(|e| OptixError::Other(e.to_string()))?;

    let mut removed = Vec::new();
    let mut failed = Vec::new();
    for o in outcomes {
        if o.ok {
            rollback::record_change(
                db.inner(),
                &snapshot.id,
                ChangeRecord {
                    id: None,
                    snapshot_id: String::new(),
                    domain: "appx".into(),
                    location: o.full_name.clone(),
                    kind: "remove".into(),
                    old_value: None,
                    new_value: None,
                    old_json: Some(serde_json::json!({
                        "name": o.name,
                        "full_name": o.full_name,
                        "install_location": o.install_location,
                        "provisioned": o.provisioned,
                    })),
                    new_json: None,
                    applied_at_ms: None,
                    verified: true,
                    rolled_back: false,
                },
            )?;
            removed.push(o.full_name);
        } else {
            failed.push(AppxRemovalFailure {
                full_name: o.full_name,
                error: o.error.unwrap_or_default(),
            });
        }
    }

    Ok(BloatwareRemoveResult {
        snapshot_id: snapshot.id.clone(),
        removed,
        failed,
    })
}
