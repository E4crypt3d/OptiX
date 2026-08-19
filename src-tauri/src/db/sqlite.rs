use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::Result;
use crate::models::games::GameProfile;
use crate::models::hardware::HardwareSample;
use crate::models::snapshot::{ChangeRecord, Snapshot, SnapshotStatus};

/// Root directory for all Optix data.
///
/// On Windows this is `C:\ProgramData\Optix`; elsewhere it falls back to a
/// per-user application data directory (used only for development on non
/// Windows hosts).
pub fn data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        let base = std::env::var("PROGRAMDATA").unwrap_or_else(|_| "C:\\ProgramData".into());
        PathBuf::from(base).join("Optix")
    }
    #[cfg(not(windows))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("optix")
    }
}

/// Directory where snapshots are stored (`<data_dir>/Snapshots`).
/// Used by the snapshot/rollback engine (Phase 2).
#[allow(dead_code)]
pub fn snapshots_dir() -> PathBuf {
    data_dir().join("Snapshots")
}

/// Schema version 1 — the full Optix data model. Migrations are applied by
/// incrementing `PRAGMA user_version`; each future phase appends a new step.
const SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS hardware_history (
    id            INTEGER PRIMARY KEY,
    ts            INTEGER NOT NULL,
    cpu_usage     REAL,
    cpu_temp      REAL,
    ram_used_mb   INTEGER,
    ram_total_mb  INTEGER,
    gpu_usage     REAL,
    gpu_temp      REAL,
    gpu_vram_mb   INTEGER,
    gpu_power_w   REAL,
    disk_used_mb  INTEGER,
    disk_total_mb INTEGER,
    net_down_bps  INTEGER,
    net_up_bps    INTEGER,
    fps           REAL,
    frame_time_ms REAL
);

CREATE TABLE IF NOT EXISTS snapshots (
    id          TEXT PRIMARY KEY,
    name        TEXT,
    reason      TEXT,
    created_at  INTEGER,
    restored_at INTEGER,
    status      TEXT
);

CREATE TABLE IF NOT EXISTS changes (
    id           INTEGER PRIMARY KEY,
    snapshot_id  TEXT NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    domain       TEXT NOT NULL,
    location     TEXT NOT NULL,
    kind         TEXT NOT NULL,
    old_value    TEXT,
    new_value    TEXT,
    old_json     TEXT,
    new_json     TEXT,
    applied_at   INTEGER,
    verified     INTEGER DEFAULT 0,
    rolled_back  INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS profiles (
    id   INTEGER PRIMARY KEY,
    name TEXT UNIQUE,
    kind TEXT,
    json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS games (
    id           INTEGER PRIMARY KEY,
    name         TEXT,
    launcher     TEXT,
    app_id       TEXT,
    install_path TEXT,
    executable   TEXT,
    last_played  INTEGER,
    detected_at  INTEGER
);

CREATE TABLE IF NOT EXISTS game_profiles (
    game_id         INTEGER PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
    cpu_priority    TEXT,
    affinity_mask   TEXT,
    power_profile   TEXT,
    network_profile TEXT,
    cleanup_bg      INTEGER DEFAULT 0,
    gpu_profile     TEXT,
    enabled         INTEGER DEFAULT 1
);

CREATE TABLE IF NOT EXISTS benchmarks (
    id                 INTEGER PRIMARY KEY,
    game_id            INTEGER REFERENCES games(id),
    game_name          TEXT,
    started_at         INTEGER,
    duration_ms        INTEGER,
    avg_fps            REAL,
    p1_fps             REAL,
    p01_fps            REAL,
    avg_frame_time_ms  REAL,
    p95_frame_time_ms  REAL,
    cpu_avg            REAL,
    gpu_avg            REAL,
    ram_avg_mb         REAL,
    latency_ms         REAL,
    config_hash        TEXT,
    csv_path           TEXT
);

CREATE TABLE IF NOT EXISTS crash_reports (
    id              INTEGER PRIMARY KEY,
    detected_at     INTEGER,
    app             TEXT,
    pid             INTEGER,
    event_id        INTEGER,
    module          TEXT,
    exception_code  TEXT,
    wer_report_path TEXT,
    minidump_path   TEXT,
    report_zip_path TEXT
);
";

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version < 1 {
        conn.execute_batch(SCHEMA_V1)?;
        conn.pragma_update(None, "user_version", 1)?;
    }
    Ok(())
}

/// Wrapper around a single SQLite connection, safe to share across Tauri
/// commands via `tauri::State`.
pub struct Database {
    conn: Mutex<Connection>,
}

/// A row from the `games` table (live running state is annotated later by
/// `engine::games`).
pub struct GameRow {
    pub id: i64,
    pub name: String,
    pub launcher: String,
    pub app_id: Option<String>,
    pub install_path: String,
    pub executable: String,
    pub last_played: Option<i64>,
    pub detected_at: Option<i64>,
}

impl Database {
    /// Open (creating if necessary) the Optix database at the default path.
    pub fn open() -> Result<Self> {
        let dir = data_dir();
        std::fs::create_dir_all(&dir)?;
        let conn = Connection::open(dir.join("optix.db"))?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory database (used by tests).
    #[cfg(test)]
    fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Insert a telemetry sample into `hardware_history`.
    pub fn insert_hardware_sample(&self, s: &HardwareSample) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO hardware_history
                (ts, cpu_usage, cpu_temp, ram_used_mb, ram_total_mb, gpu_usage, gpu_temp,
                 gpu_vram_mb, gpu_power_w, disk_used_mb, disk_total_mb, net_down_bps,
                 net_up_bps, fps, frame_time_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            rusqlite::params![
                s.ts_ms,
                s.cpu_usage,
                s.cpu_temp,
                s.ram_used_mb,
                s.ram_total_mb,
                s.gpu_usage,
                s.gpu_temp,
                s.gpu_vram_mb,
                s.gpu_power_w,
                s.disk_used_mb,
                s.disk_total_mb,
                s.net_down_bps,
                s.net_up_bps,
                s.fps,
                s.frame_time_ms,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Return the most recent telemetry samples, newest first.
    pub fn recent_hardware_samples(&self, limit: i64) -> Result<Vec<HardwareSample>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, ts, cpu_usage, cpu_temp, ram_used_mb, ram_total_mb, gpu_usage,
                    gpu_temp, gpu_vram_mb, gpu_power_w, disk_used_mb, disk_total_mb,
                    net_down_bps, net_up_bps, fps, frame_time_ms
             FROM hardware_history ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |row| {
            Ok(HardwareSample {
                id: Some(row.get(0)?),
                ts_ms: row.get(1)?,
                cpu_usage: row.get(2)?,
                cpu_temp: row.get(3)?,
                ram_used_mb: row.get(4)?,
                ram_total_mb: row.get(5)?,
                gpu_usage: row.get(6)?,
                gpu_temp: row.get(7)?,
                gpu_vram_mb: row.get(8)?,
                gpu_power_w: row.get(9)?,
                disk_used_mb: row.get(10)?,
                disk_total_mb: row.get(11)?,
                net_down_bps: row.get(12)?,
                net_up_bps: row.get(13)?,
                fps: row.get(14)?,
                frame_time_ms: row.get(15)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn insert_snapshot(&self, s: &Snapshot) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO snapshots (id, name, reason, created_at, restored_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                s.id,
                s.name,
                s.reason,
                s.created_at_ms,
                s.restored_at_ms,
                s.status.as_str(),
            ],
        )?;
        Ok(())
    }

    /// List snapshots, newest first.
    pub fn list_snapshots(&self) -> Result<Vec<Snapshot>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, reason, created_at, restored_at, status FROM snapshots
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], snapshot_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_snapshot(&self, id: &str) -> Result<Option<Snapshot>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, reason, created_at, restored_at, status FROM snapshots
             WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map([id], snapshot_from_row)?;
        rows.next().transpose().map_err(Into::into)
    }

    pub fn update_snapshot_status(&self, id: &str, status: SnapshotStatus) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE snapshots SET status = ?1 WHERE id = ?2",
            rusqlite::params![status.as_str(), id],
        )?;
        Ok(())
    }

    pub fn delete_snapshot(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM snapshots WHERE id = ?1", [id])?;
        Ok(())
    }

    // Invoked by engine::rollback::record_change once mutation phases (cleanup,
    // power, services) start writing changes.
    #[allow(dead_code)]
    pub fn insert_change(&self, c: &ChangeRecord) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO changes
                (snapshot_id, domain, location, kind, old_value, new_value, old_json,
                 new_json, applied_at, verified, rolled_back)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                c.snapshot_id,
                c.domain,
                c.location,
                c.kind,
                c.old_value,
                c.new_value,
                c.old_json.as_ref().map(|v| v.to_string()),
                c.new_json.as_ref().map(|v| v.to_string()),
                c.applied_at_ms,
                c.verified,
                c.rolled_back,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Insert a game into the library.
    pub fn insert_game(
        &self,
        name: &str,
        launcher: &str,
        app_id: Option<&str>,
        install_path: &str,
        executable: &str,
        detected_at: i64,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO games (name, launcher, app_id, install_path, executable, last_played, detected_at)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)",
            rusqlite::params![name, launcher, app_id, install_path, executable, detected_at],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// List all games, sorted by name.
    pub fn list_games(&self) -> Result<Vec<GameRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, launcher, app_id, install_path, executable, last_played, detected_at
             FROM games ORDER BY name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(GameRow {
                id: row.get(0)?,
                name: row.get(1)?,
                launcher: row.get(2)?,
                app_id: row.get(3)?,
                install_path: row.get(4)?,
                executable: row.get(5)?,
                last_played: row.get(6)?,
                detected_at: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Fetch a single game by id.
    pub fn get_game(&self, id: i64) -> Result<Option<GameRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, launcher, app_id, install_path, executable, last_played, detected_at
             FROM games WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map([id], |row| {
            Ok(GameRow {
                id: row.get(0)?,
                name: row.get(1)?,
                launcher: row.get(2)?,
                app_id: row.get(3)?,
                install_path: row.get(4)?,
                executable: row.get(5)?,
                last_played: row.get(6)?,
                detected_at: row.get(7)?,
            })
        })?;
        rows.next().transpose().map_err(Into::into)
    }

    /// Delete a game (its profile is removed via ON DELETE CASCADE).
    pub fn delete_game(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM games WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Fetch a game's profile, if one has been saved.
    pub fn get_game_profile(&self, game_id: i64) -> Result<Option<GameProfile>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT game_id, cpu_priority, affinity_mask, power_profile, network_profile,
                    cleanup_bg, gpu_profile, enabled
             FROM game_profiles WHERE game_id = ?1",
        )?;
        let mut rows = stmt.query_map([game_id], |row| {
            Ok(GameProfile {
                game_id: row.get(0)?,
                cpu_priority: row.get(1)?,
                affinity_mask: row.get(2)?,
                power_profile: row.get(3)?,
                network_profile: row.get(4)?,
                cleanup_bg: row.get(5)?,
                gpu_profile: row.get(6)?,
                enabled: row.get(7)?,
            })
        })?;
        rows.next().transpose().map_err(Into::into)
    }

    /// Insert or update a game profile.
    pub fn save_game_profile(&self, p: &GameProfile) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO game_profiles
                (game_id, cpu_priority, affinity_mask, power_profile, network_profile,
                 cleanup_bg, gpu_profile, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(game_id) DO UPDATE SET
                cpu_priority = excluded.cpu_priority,
                affinity_mask = excluded.affinity_mask,
                power_profile = excluded.power_profile,
                network_profile = excluded.network_profile,
                cleanup_bg = excluded.cleanup_bg,
                gpu_profile = excluded.gpu_profile,
                enabled = excluded.enabled",
            rusqlite::params![
                p.game_id,
                p.cpu_priority,
                p.affinity_mask,
                p.power_profile,
                p.network_profile,
                p.cleanup_bg,
                p.gpu_profile,
                p.enabled,
            ],
        )?;
        Ok(())
    }

    /// List a snapshot's changes in application order.
    pub fn list_changes(&self, snapshot_id: &str) -> Result<Vec<ChangeRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, snapshot_id, domain, location, kind, old_value, new_value,
                    old_json, new_json, applied_at, verified, rolled_back
             FROM changes WHERE snapshot_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([snapshot_id], |row| {
            let old_json: Option<String> = row.get(7)?;
            let new_json: Option<String> = row.get(8)?;
            Ok(ChangeRecord {
                id: row.get(0)?,
                snapshot_id: row.get(1)?,
                domain: row.get(2)?,
                location: row.get(3)?,
                kind: row.get(4)?,
                old_value: row.get(5)?,
                new_value: row.get(6)?,
                old_json: old_json.and_then(|s| serde_json::from_str(&s).ok()),
                new_json: new_json.and_then(|s| serde_json::from_str(&s).ok()),
                applied_at_ms: row.get(9)?,
                verified: row.get(10)?,
                rolled_back: row.get(11)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

fn snapshot_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Snapshot> {
    Ok(Snapshot {
        id: row.get(0)?,
        name: row.get(1)?,
        reason: row.get(2)?,
        created_at_ms: row.get(3)?,
        restored_at_ms: row.get(4)?,
        status: SnapshotStatus::from_str(&row.get::<_, String>(5)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_creates_all_tables() {
        let db = Database::open_in_memory().expect("open in-memory database");
        let conn = db.conn.lock().unwrap();

        let expected = [
            "hardware_history",
            "snapshots",
            "changes",
            "profiles",
            "games",
            "game_profiles",
            "benchmarks",
            "crash_reports",
        ];
        for table in expected {
            let count: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing table {table}");
        }

        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn hardware_sample_round_trip() {
        let db = Database::open_in_memory().unwrap();
        let sample = HardwareSample {
            id: None,
            ts_ms: 123456,
            cpu_usage: Some(12.5),
            cpu_temp: None,
            ram_used_mb: Some(8000),
            ram_total_mb: Some(16000),
            gpu_usage: None,
            gpu_temp: None,
            gpu_vram_mb: None,
            gpu_power_w: None,
            disk_used_mb: Some(100),
            disk_total_mb: Some(1000),
            net_down_bps: Some(500),
            net_up_bps: Some(200),
            fps: None,
            frame_time_ms: None,
        };
        let id = db.insert_hardware_sample(&sample).unwrap();
        assert!(id > 0);
        let rows = db.recent_hardware_samples(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ts_ms, 123456);
        assert_eq!(rows[0].cpu_usage, Some(12.5));
        assert_eq!(rows[0].ram_total_mb, Some(16000));
        assert_eq!(rows[0].net_down_bps, Some(500));
    }

    #[test]
    fn snapshot_and_change_round_trip() {
        let db = Database::open_in_memory().unwrap();
        let s = Snapshot {
            id: "abc".into(),
            name: "before cleanup".into(),
            reason: Some("test".into()),
            created_at_ms: 1,
            restored_at_ms: None,
            status: SnapshotStatus::Active,
        };
        db.insert_snapshot(&s).unwrap();

        let c = ChangeRecord {
            id: None,
            snapshot_id: "abc".into(),
            domain: "registry".into(),
            location: r"HKLM\SOFTWARE\Test\Value".into(),
            kind: "set".into(),
            old_value: Some("1".into()),
            new_value: Some("2".into()),
            old_json: None,
            new_json: None,
            applied_at_ms: Some(2),
            verified: true,
            rolled_back: false,
        };
        db.insert_change(&c).unwrap();

        assert_eq!(db.list_snapshots().unwrap().len(), 1);
        let changes = db.list_changes("abc").unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].old_value.as_deref(), Some("1"));
        assert!(changes[0].verified);

        db.delete_snapshot("abc").unwrap();
        assert!(db.list_snapshots().unwrap().is_empty());
        // CASCADE removes the change row too.
        assert!(db.list_changes("abc").unwrap().is_empty());
    }

    #[test]
    fn migration_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }
}
