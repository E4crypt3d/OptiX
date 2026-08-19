// Forward-declared for the snapshot/rollback engine (Phase 2).
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Lifecycle state of a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotStatus {
    Active,
    Restored,
    Deleted,
}

impl SnapshotStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SnapshotStatus::Active => "active",
            SnapshotStatus::Restored => "restored",
            SnapshotStatus::Deleted => "deleted",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "restored" => SnapshotStatus::Restored,
            "deleted" => SnapshotStatus::Deleted,
            _ => SnapshotStatus::Active,
        }
    }
}

/// A point-in-time capture of every system area Optix touches. Stored as a
/// set of JSON files under `<data_dir>/Snapshots/<id>/` and a row in `snapshots`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: String,
    pub name: String,
    pub reason: Option<String>,
    pub created_at_ms: i64,
    pub restored_at_ms: Option<i64>,
    pub status: SnapshotStatus,
}

/// A single reversible mutation, mirroring the `changes` table. `old_*` /
/// `new_*` hold the before/after values so the rollback engine can restore
/// them in reverse order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRecord {
    pub id: Option<i64>,
    pub snapshot_id: String,
    /// One of: registry, power, service, network, startup, process, file, gpu.
    pub domain: String,
    /// Exact key/name/path that was modified.
    pub location: String,
    /// One of: set, delete, start, stop, disable, kill, replace.
    pub kind: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub old_json: Option<serde_json::Value>,
    pub new_json: Option<serde_json::Value>,
    pub applied_at_ms: Option<i64>,
    pub verified: bool,
    pub rolled_back: bool,
}
