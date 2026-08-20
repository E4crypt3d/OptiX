use std::fs;
use std::sync::Mutex;

use serde_json::{json, Value};

use crate::db::sqlite::{snapshots_dir, Database};
use crate::engine::snapshot::validate_snapshot_id;
use crate::error::{OptixError, Result};
use crate::models::snapshot::ChangeRecord;

/// Serializes restores so two concurrent restores of the same snapshot can't
/// double-apply — some domains (appx reinstalls, GPU profile removal) are not
/// idempotent.
static RESTORE_LOCK: Mutex<()> = Mutex::new(());

/// Record a change against a snapshot. Called by the mutation phases (cleanup,
/// power, services, …) as they apply changes. The DB is the single source of
/// truth — the on-disk snapshot JSON holds captured state only, so there's no
/// side file to keep in sync (or race on).
pub fn record_change(
    db: &Database,
    snapshot_id: &str,
    mut change: ChangeRecord,
) -> Result<ChangeRecord> {
    change.snapshot_id = snapshot_id.to_string();
    change.applied_at_ms = Some(super::now_ms() as i64);
    db.insert_change(&change)?;
    Ok(change)
}

/// Load a snapshot's changes, validating that the snapshot exists.
pub fn load(db: &Database, snapshot_id: &str) -> Result<Vec<ChangeRecord>> {
    validate_snapshot_id(snapshot_id)?;
    db.get_snapshot(snapshot_id)?
        .ok_or_else(|| OptixError::InvalidState(format!("snapshot {snapshot_id} not found")))?;
    db.list_changes(snapshot_id)
}

/// Apply a snapshot's changes in reverse order. Every change is attempted — a
/// failure in one doesn't abort the rest, so a single bad entry can't leave
/// the remaining reversions unapplied. Returns the number of changes actually
/// reverted; `Err` lists the failures when any occurred.
pub fn apply_reverse(changes: &[ChangeRecord]) -> Result<usize> {
    let _guard = RESTORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut reverted = 0usize;
    let mut failures = Vec::new();
    for change in changes.iter().rev() {
        match rollback_change(change) {
            Ok(RollbackOutcome::Reverted) => reverted += 1,
            Ok(RollbackOutcome::Skipped) => {}
            Err(e) => failures.push(format!("{}: {e}", change.location)),
        }
    }
    if failures.is_empty() {
        Ok(reverted)
    } else {
        Err(OptixError::InvalidState(format!(
            "restored {reverted} of {} changes; failed: {}",
            changes.len(),
            failures.join("; ")
        )))
    }
}

/// Restore a snapshot: load its changes, apply them in reverse order, and —
/// only when every one succeeded — mark the snapshot restored with a
/// timestamp. On partial failure the snapshot stays active so the user can
/// retry.
pub fn restore(db: &Database, snapshot_id: &str) -> Result<usize> {
    let changes = load(db, snapshot_id)?;
    let reverted = apply_reverse(&changes)?;
    db.mark_snapshot_restored(snapshot_id, super::now_ms() as i64)?;
    Ok(reverted)
}

enum RollbackOutcome {
    /// The change was actually reversed.
    Reverted,
    /// Recorded for audit but not reversible (file deletions) — counted as
    /// neither reverted nor failed.
    Skipped,
}

fn rollback_change(change: &ChangeRecord) -> Result<RollbackOutcome> {
    let outcome = match change.domain.as_str() {
        "registry" => {
            crate::win::registry::rollback_registry(change)?;
            RollbackOutcome::Reverted
        }
        "power" => {
            crate::win::power::rollback_power(change)?;
            RollbackOutcome::Reverted
        }
        "service" => {
            crate::win::services::rollback_service(change)?;
            RollbackOutcome::Reverted
        }
        "gpu" => {
            crate::win::gpu::rollback_gpu(change)?;
            RollbackOutcome::Reverted
        }
        "appx" => {
            crate::win::appx::rollback_appx(change)?;
            RollbackOutcome::Reverted
        }
        // File deletions (cleanup / shader caches) are recorded for audit but
        // not reversible — restoring a snapshot skips them instead of failing.
        "file" => RollbackOutcome::Skipped,
        other => {
            return Err(OptixError::InvalidState(format!(
                "rollback not implemented for domain '{other}'"
            )))
        }
    };
    Ok(outcome)
}

/// Structural diff of two snapshots' JSON files.
pub fn diff(db: &Database, a: &str, b: &str) -> Result<Value> {
    for id in [a, b] {
        validate_snapshot_id(id)?;
        if db.get_snapshot(id)?.is_none() {
            return Err(OptixError::InvalidState(format!("snapshot {id} not found")));
        }
    }
    let base = snapshots_dir();
    let a_map = load_dir(&base.join(a))?;
    let b_map = load_dir(&base.join(b))?;
    Ok(diff_values(&a_map, &b_map, "$"))
}

/// Load every `*.json` file in a snapshot directory into an object keyed by
/// file stem.
fn load_dir(dir: &std::path::Path) -> Result<Value> {
    let mut map = serde_json::Map::new();
    if !dir.is_dir() {
        return Ok(Value::Object(map));
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            // The change ledger and creation timestamp are per-snapshot
            // bookkeeping, not captured state — including them would make
            // every diff show constant `changed` noise between snapshots.
            if matches!(name.as_str(), "changes" | "timestamp") {
                continue;
            }
            if let Ok(text) = fs::read_to_string(&path) {
                if let Ok(val) = serde_json::from_str(&text) {
                    map.insert(name, val);
                }
            }
        }
    }
    Ok(Value::Object(map))
}

fn diff_values(a: &Value, b: &Value, path: &str) -> Value {
    let mut changes = Vec::new();
    diff_into(a, b, path, &mut changes);
    Value::Array(changes)
}

fn diff_into(a: &Value, b: &Value, path: &str, out: &mut Vec<Value>) {
    match (a, b) {
        (Value::Object(am), Value::Object(bm)) => {
            for (k, av) in am {
                let p = format!("{path}.{k}");
                match bm.get(k) {
                    Some(bv) => diff_into(av, bv, &p, out),
                    None => out.push(json!({ "path": p, "kind": "removed", "old": av })),
                }
            }
            for (k, bv) in bm {
                if !am.contains_key(k) {
                    out.push(json!({ "path": format!("{path}.{k}"), "kind": "added", "new": bv }));
                }
            }
        }
        _ if a != b => out.push(json!({ "path": path, "kind": "changed", "old": a, "new": b })),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_path_traversal_ids() {
        for bad in ["../etc/passwd", "a/../b", "..", "a\\b", ""] {
            assert!(
                validate_snapshot_id(bad).is_err(),
                "id {bad:?} should be rejected"
            );
        }
        // A real UUID-shaped id (the only kind the app generates) passes.
        assert!(validate_snapshot_id("3f1d2e8a-9c4b-4f6a-8e2b-1a2b3c4d5e6f").is_ok());
        assert!(validate_snapshot_id("s1").is_ok());
    }

    #[test]
    fn diff_detects_changes() {
        let a = json!({ "registry.json": { "HAGS": "1" }, "system.json": { "host": "pc" } });
        let b = json!({ "registry.json": { "HAGS": "2" }, "system.json": { "host": "pc", "new": true } });
        let d = diff_values(&a, &b, "$");
        let arr = d.as_array().unwrap();
        assert!(arr.iter().any(|c| c["path"] == "$.registry.json.HAGS" && c["kind"] == "changed"));
        assert!(arr.iter().any(|c| c["path"] == "$.system.json.new" && c["kind"] == "added"));
    }

    #[test]
    fn diff_ignores_equal_values() {
        let a = json!({ "x": 1 });
        let b = json!({ "x": 1 });
        assert_eq!(diff_values(&a, &b, "$").as_array().unwrap().len(), 0);
    }

    #[test]
    fn load_dir_excludes_bookkeeping_files() {
        let dir = std::env::temp_dir().join(format!("optix-rollback-diff-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("system.json"), r#"{"host":"pc"}"#).unwrap();
        fs::write(dir.join("registry.json"), r#"{"HAGS":"1"}"#).unwrap();
        fs::write(dir.join("changes.json"), "[1]").unwrap();
        fs::write(dir.join("timestamp.json"), r#"{"created_at_ms":1}"#).unwrap();
        let map = load_dir(&dir).unwrap();
        let obj = map.as_object().unwrap();
        assert!(obj.contains_key("system"));
        assert!(obj.contains_key("registry"));
        assert!(!obj.contains_key("changes"));
        assert!(!obj.contains_key("timestamp"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn restore_skips_file_changes_and_marks_restored() {
        use crate::db::sqlite::Database;
        use crate::models::snapshot::{Snapshot, SnapshotStatus};

        let db = Database::open_in_memory().unwrap();
        db.insert_snapshot(&Snapshot {
            id: "s1".into(),
            name: "cleanup".into(),
            reason: None,
            created_at_ms: 1,
            restored_at_ms: None,
            status: SnapshotStatus::Active,
        })
        .unwrap();
        db.insert_change(&ChangeRecord {
            id: None,
            snapshot_id: "s1".into(),
            domain: "file".into(),
            location: "/tmp/x".into(),
            kind: "delete".into(),
            old_value: None,
            new_value: None,
            old_json: None,
            new_json: None,
            applied_at_ms: Some(2),
            verified: true,
            rolled_back: false,
        })
        .unwrap();

        // File deletions are audit-only: nothing to revert, status still flips.
        assert_eq!(restore(&db, "s1").unwrap(), 0);
        let s = db.get_snapshot("s1").unwrap().unwrap();
        assert_eq!(s.status, SnapshotStatus::Restored);
        assert!(s.restored_at_ms.is_some());
    }

    #[test]
    fn restore_reports_partial_failures_and_stays_active() {
        use crate::db::sqlite::Database;
        use crate::models::snapshot::{Snapshot, SnapshotStatus};

        let db = Database::open_in_memory().unwrap();
        db.insert_snapshot(&Snapshot {
            id: "s2".into(),
            name: "t".into(),
            reason: None,
            created_at_ms: 1,
            restored_at_ms: None,
            status: SnapshotStatus::Active,
        })
        .unwrap();
        db.insert_change(&ChangeRecord {
            id: None,
            snapshot_id: "s2".into(),
            domain: "bogus".into(),
            location: "HKLM\\X".into(),
            kind: "set".into(),
            old_value: Some("1".into()),
            new_value: Some("2".into()),
            old_json: None,
            new_json: None,
            applied_at_ms: Some(2),
            verified: true,
            rolled_back: false,
        })
        .unwrap();

        let err = restore(&db, "s2").unwrap_err();
        assert!(err.to_string().contains("restored 0 of 1 changes"));
        assert!(err.to_string().contains("bogus"));
        // Partial restore: snapshot stays active so the user can retry.
        let s = db.get_snapshot("s2").unwrap().unwrap();
        assert_eq!(s.status, SnapshotStatus::Active);
        assert!(s.restored_at_ms.is_none());
    }
}
