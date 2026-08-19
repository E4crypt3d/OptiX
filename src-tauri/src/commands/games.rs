use std::sync::Arc;

use tauri::State;

use crate::db::sqlite::Database;
use crate::engine::game_watcher::GameWatcher;
use crate::engine::{games, power};
use crate::error::{OptixError, Result};
use crate::models::games::{DetectedGame, Game, GameProfile, GameProfileApplyResult};

/// Scan installed launchers for games (read-only, nothing saved yet).
#[tauri::command]
pub fn detect_games() -> Vec<DetectedGame> {
    games::detect_all()
}

/// List the saved game library, annotated with running/boosted state.
#[tauri::command]
pub fn list_games(
    db: State<'_, Database>,
    watcher: State<'_, Arc<GameWatcher>>,
) -> Result<Vec<Game>> {
    games::list_games(db.inner(), Some(watcher.inner()))
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

/// Remove a game from the library.
#[tauri::command]
pub fn remove_game(db: State<'_, Database>, id: i64) -> Result<()> {
    if db.get_game(id)?.is_none() {
        return Err(OptixError::InvalidState(format!("game {id} not found")));
    }
    db.delete_game(id)
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

    let (snapshot_id, power_applied) = if profile.power_profile != "none" {
        let res = power::apply_profile(db.inner(), &profile.power_profile)?;
        (Some(res.snapshot_id), Some(res.scheme_name))
    } else {
        (None, None)
    };

    let outcome = watcher.apply_game(&game, &profile);
    Ok(GameProfileApplyResult {
        snapshot_id,
        power_applied,
        boosted: outcome.boosted,
        lowered: outcome.lowered,
        affinity_applied: outcome.affinity,
    })
}

/// Force-restore a game's boosted processes (usually automatic on exit).
#[tauri::command]
pub fn restore_game_profile(
    watcher: State<'_, Arc<GameWatcher>>,
    game_id: i64,
) -> Result<usize> {
    Ok(watcher.restore_game(game_id))
}
