use std::sync::Arc;

use tauri::State;

use crate::db::sqlite::Database;
use crate::engine::game_watcher::GameWatcher;
use crate::engine::{games, power, rollback, snapshot};
use crate::error::{OptixError, Result};
use crate::models::games::{DetectedGame, Game, GameProfile, GameProfileApplyResult};
use crate::models::snapshot::ChangeRecord;

/// Scan installed launchers for games (read-only, nothing saved yet).
/// File/registry scanning runs off the main thread.
#[tauri::command]
pub async fn detect_games() -> Result<Vec<DetectedGame>> {
    tauri::async_runtime::spawn_blocking(games::detect_all)
        .await
        .map_err(|e| OptixError::Other(e.to_string()))
}

/// List the saved game library, annotated with running/boosted state.
/// Process enumeration runs off the main thread (the UI polls this).
#[tauri::command]
pub async fn list_games(
    db: State<'_, Database>,
    watcher: State<'_, Arc<GameWatcher>>,
) -> Result<Vec<Game>> {
    let processes = tauri::async_runtime::spawn_blocking(games::running_process_names)
        .await
        .map_err(|e| OptixError::Other(e.to_string()))?;
    games::list_games(db.inner(), Some(watcher.inner()), processes)
}

/// Add a detected game to the library (dedup + default profile).
#[tauri::command]
pub fn add_game(
    db: State<'_, Database>,
    launcher: String,
    app_id: Option<String>,
    name: String,
    install_path: String,
    executable: String,
) -> Result<Game> {
    games::add_game(
        db.inner(),
        &launcher,
        app_id.as_deref(),
        &name,
        &install_path,
        &executable,
    )
}

/// Add a manually-chosen executable as a game.
#[tauri::command]
pub fn add_manual_game(
    db: State<'_, Database>,
    name: String,
    executable: String,
) -> Result<Game> {
    let install_path = std::path::Path::new(&executable)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    games::add_game(db.inner(), "manual", None, &name, &install_path, &executable)
}

/// Remove a game from the library (best-effort DRS profile cleanup).
#[tauri::command]
pub fn remove_game(db: State<'_, Database>, id: i64) -> Result<()> {
    let row = db
        .get_game(id)?
        .ok_or_else(|| OptixError::InvalidState(format!("game {id} not found")))?;
    let had_nvidia_profile = db
        .get_game_profile(id)?
        .is_some_and(|p| p.gpu_profile.as_deref() == Some("nvidia"));
    db.delete_game(id)?;
    // Don't leave driver-settings profiles behind for removed games.
    if had_nvidia_profile {
        if let Err(e) = crate::win::nvapi::remove_profile(&row.name) {
            crate::logging::warn(&format!("failed to remove DRS profile for {}: {e}", row.name));
        }
    }
    Ok(())
}

/// Fetch a game's profile (the default profile when none is saved yet).
#[tauri::command]
pub fn get_game_profile(db: State<'_, Database>, game_id: i64) -> Result<GameProfile> {
    if db.get_game(game_id)?.is_none() {
        return Err(OptixError::InvalidState(format!("game {game_id} not found")));
    }
    Ok(db
        .get_game_profile(game_id)?
        .unwrap_or_else(|| games::default_profile(game_id)))
}

/// Save (upsert) a game profile after validation.
#[tauri::command]
pub fn save_game_profile(db: State<'_, Database>, profile: GameProfile) -> Result<()> {
    if db.get_game(profile.game_id)?.is_none() {
        return Err(OptixError::InvalidState(format!(
            "game {} not found",
            profile.game_id
        )));
    }
    games::validate_profile(&profile)?;
    db.save_game_profile(&profile)
}

/// Apply a game profile now: power profile (snapshot-first, reversible) plus
/// priority/affinity/background lowering for any running instance.
#[tauri::command]
pub fn apply_game_profile(
    db: State<'_, Database>,
    watcher: State<'_, Arc<GameWatcher>>,
    game_id: i64,
) -> Result<GameProfileApplyResult> {
    let row = db
        .get_game(game_id)?
        .ok_or_else(|| OptixError::InvalidState(format!("game {game_id} not found")))?;
    let profile = db
        .get_game_profile(game_id)?
        .unwrap_or_else(|| games::default_profile(game_id));
    games::validate_profile(&profile)?;
    let game = games::row_to_game(&row);

    let (mut snapshot_id, power_applied) = if profile.power_profile != "none" {
        let res = power::apply_profile(db.inner(), &profile.power_profile)?;
        (Some(res.snapshot_id), Some(res.scheme_name))
    } else {
        (None, None)
    };

    let outcome = watcher.apply_game(&game, &profile);

    // NVIDIA DRS per-game profile (best-effort: only on NVIDIA hardware with
    // `gpu_profile: nvidia`; failures surface in the UI, never fail the apply).
    // The created profile is recorded in the apply snapshot so Rollback
    // Center can remove it (restored via the `gpu` rollback domain).
    let gpu_profile = if profile.gpu_profile.as_deref() == Some("nvidia") {
        let opts = crate::win::nvapi::DrsOptions {
            prefer_max_performance: profile.power_profile != "none",
            shader_cache_on: true,
        };
        let exe = (!game.executable.is_empty()).then(|| game.executable.as_str());
        match crate::win::nvapi::apply_profile(&game.name, exe, &opts) {
            Ok(r) => {
                let snap_id = match &snapshot_id {
                    Some(id) => id.clone(),
                    None => {
                        let s = snapshot::create_lightweight(
                            db.inner(),
                            "NVIDIA DRS profile",
                            Some(&game.name),
                        )?;
                        snapshot_id = Some(s.id.clone());
                        s.id
                    }
                };
                rollback::record_change(
                    db.inner(),
                    &snap_id,
                    ChangeRecord {
                        id: None,
                        snapshot_id: String::new(),
                        domain: "gpu".to_string(),
                        location: game.name.clone(),
                        kind: "nvapi_profile".to_string(),
                        old_value: None,
                        new_value: Some(r.profile.clone()),
                        old_json: None,
                        new_json: None,
                        applied_at_ms: None,
                        verified: true,
                        rolled_back: false,
                    },
                )?;
                Some(r.profile)
            }
            Err(e) => {
                // Best-effort for end users (e.g. no NVIDIA driver), but the
                // real failure is logged so developers see it.
                crate::logging::warn(&format!("NVIDIA DRS profile for {} not applied: {e}", game.name));
                None
            }
        }
    } else {
        None
    };

    Ok(GameProfileApplyResult {
        snapshot_id,
        power_applied,
        boosted: outcome.boosted,
        lowered: outcome.lowered,
        affinity_applied: outcome.affinity,
        gpu_profile,
    })
}

/// Remove the NVIDIA DRS profile Optix created for a game (the rollback path
/// for the per-game GPU profile).
#[tauri::command]
pub fn remove_game_drs_profile(game_name: String) -> Result<()> {
    crate::win::nvapi::remove_profile(&game_name)
}

/// Force-restore a game's boosted processes (usually automatic on exit).
#[tauri::command]
pub fn restore_game_profile(
    watcher: State<'_, Arc<GameWatcher>>,
    game_id: i64,
) -> Result<usize> {
    Ok(watcher.restore_game(game_id))
}
