use serde::Serialize;

/// A scannable cleanup category (temp files, caches, crash dumps, logs).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupCategory {
    pub id: String,
    pub name: String,
    pub description: String,
    /// "safe" | "caution"
    pub safety: String,
    pub size_bytes: u64,
    pub file_count: u64,
    /// True when the files will be rebuilt on next launch (e.g. shader caches).
    pub expected_rebuild: bool,
}

/// Per-category cleanup outcome.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryResult {
    pub id: String,
    pub before_bytes: u64,
    pub freed_bytes: u64,
    pub files_removed: u64,
    pub files_skipped: u64,
}

/// Overall cleanup outcome, tied to the snapshot created before deletion.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    pub snapshot_id: String,
    pub freed_bytes: u64,
    pub files_removed: u64,
    pub files_skipped: u64,
    pub categories: Vec<CategoryResult>,
}
