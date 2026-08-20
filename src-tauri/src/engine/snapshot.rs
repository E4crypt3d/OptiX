use std::fs;

use serde_json::Value;

use crate::db::sqlite::{snapshots_dir, Database};
use crate::error::Result;
use crate::models::hardware::HardwareInfo;
use crate::models::snapshot::{Snapshot, SnapshotStatus};

/// Number of snapshots to retain (oldest pruned after each create).
pub const SNAPSHOT_RETENTION: usize = 20;

/// Capture a full snapshot (system fingerprint from a scan) to disk + DB.
pub fn create(
    db: &Database,
    name: &str,
    reason: Option<&str>,
    system: &HardwareInfo,
) -> Result<Snapshot> {
    create_with_system(db, name, reason, serde_json::to_value(system)?)
}

/// Capture a lightweight snapshot (no full scan) for fast, destructive ops
/// such as cleanup.
pub fn create_lightweight(db: &Database, name: &str, reason: Option<&str>) -> Result<Snapshot> {
    let system = serde_json::json!({
        "os": sysinfo::System::name().unwrap_or_default(),
        "host_name": sysinfo::System::host_name().unwrap_or_default(),
        "created_at_ms": super::now_ms(),
    });
    create_with_system(db, name, reason, system)
}

fn create_with_system(db: &Database, name: &str, reason: Option<&str>, system: Value) -> Result<Snapshot> {
    let id = uuid::Uuid::new_v4().to_string();
    let dir = snapshots_dir().join(&id);
    fs::create_dir_all(&dir)?;

    let created_at_ms = super::now_ms() as i64;
    fs::write(dir.join("system.json"), serde_json::to_string_pretty(&system)?)?;
    fs::write(
        dir.join("registry.json"),
        serde_json::to_string_pretty(&crate::win::registry::capture_gaming_toggles())?,
    )?;
    fs::write(
        dir.join("timestamp.json"),
        serde_json::json!({ "created_at_ms": created_at_ms }).to_string(),
    )?;

    let snapshot = Snapshot {
        id,
        name: name.to_string(),
        reason: reason.map(str::to_string),
        created_at_ms,
        restored_at_ms: None,
        status: SnapshotStatus::Active,
    };
    db.insert_snapshot(&snapshot)?;
    prune(db, SNAPSHOT_RETENTION)?;
    Ok(snapshot)
}

/// Delete a snapshot: remove its files and database row.
pub fn delete(db: &Database, id: &str) -> Result<()> {
    let dir = snapshots_dir().join(id);
    if dir.is_dir() {
        fs::remove_dir_all(&dir)?;
    }
    db.delete_snapshot(id)?;
    Ok(())
}

/// Prune snapshots beyond `keep`, oldest first.
pub fn prune(db: &Database, keep: usize) -> Result<usize> {
    let snapshots = db.list_snapshots()?;
    if snapshots.len() <= keep {
        return Ok(0);
    }
    // list_snapshots is newest-first; drop everything after the newest `keep`.
    let mut removed = 0;
    for s in snapshots.iter().skip(keep) {
        delete(db, &s.id)?;
        removed += 1;
    }
    Ok(removed)
}
