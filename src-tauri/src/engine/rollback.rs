use std::fs;

use serde_json::{json, Value};

use crate::db::sqlite::{snapshots_dir, Database};
use crate::error::{OptixError, Result};
use crate::models::snapshot::{ChangeRecord, SnapshotStatus};

/// Record a change against a snapshot: append to `changes.json` and the DB.
/// Called by the mutation phases (cleanup, power, services, …) as they apply
/// changes.
#[allow(dead_code)]
pub fn record_change(
    db: &Database,
    snapshot_id: &str,
    mut change: ChangeRecord,
) -> Result<ChangeRecord> {
    change.snapshot_id = snapshot_id.to_string();
    change.applied_at_ms = Some(super::now_ms() as i64);

    let path = changes_file(snapshot_id);
    let mut changes: Vec<ChangeRecord> = if path.exists() {
        serde_json::from_str(&fs::read_to_string(&path)?)?
    } else {
        Vec::new()
    };
    changes.push(change.clone());
    fs::write(&path, serde_json::to_string_pretty(&changes)?)?;
    db.insert_change(&change)?;
    Ok(change)
}

/// Restore a snapshot: apply its changes in reverse order.
pub fn restore(db: &Database, snapshot_id: &str) -> Result<usize> {
    let snapshot = db
        .get_snapshot(snapshot_id)?
        .ok_or_else(|| OptixError::InvalidState(format!("snapshot {snapshot_id} not found")))?;

    let changes = db.list_changes(snapshot_id)?;
    let mut restored = 0usize;
    for change in changes.iter().rev() {
        rollback_change(change)?;
        restored += 1;
    }

    db.update_snapshot_status(&snapshot.id, SnapshotStatus::Restored)?;
    Ok(restored)
}

fn rollback_change(change: &ChangeRecord) -> Result<()> {
    match change.domain.as_str() {
        "registry" => crate::win::registry::rollback_registry(change),
        "power" => crate::win::power::rollback_power(change),
        "service" => crate::win::services::rollback_service(change),
        "appx" => crate::win::appx::rollback_appx(change),
        // File deletions (cleanup / shader caches) are recorded for audit but
        // not reversible — restoring a snapshot skips them instead of failing.
        "file" => Ok(()),
        other => Err(OptixError::InvalidState(format!(
            "rollback not implemented for domain '{other}'"
        ))),
    }
}

/// Structural diff of two snapshots' JSON files.
pub fn diff(db: &Database, a: &str, b: &str) -> Result<Value> {
    for id in [a, b] {
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

#[allow(dead_code)]
fn changes_file(snapshot_id: &str) -> std::path::PathBuf {
    snapshots_dir().join(snapshot_id).join("changes.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
