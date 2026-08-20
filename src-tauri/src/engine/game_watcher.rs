//! Game-mode watcher: polls running processes and auto-applies an enabled
//! game's CPU priority + affinity (and lowers SAFE background processes) when
//! the game launches, restoring everything when it exits.
//!
//! Priority/affinity changes are in-memory and reversed by PID — a snapshot
//! can't restore a process that has exited, so these are tracked here rather
//! than via the rollback engine.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::db::sqlite::Database;
use crate::engine::{games, processes::classify};
use crate::models::games::{AffinityChange, Game, GameProfile};
use crate::models::process::{PriorityChange, PriorityClass, ProcessClass};
use crate::win;

/// One process whose priority/affinity Optix changed, with its original state.
struct WatchedProcess {
    pid: u32,
    original_priority: Option<PriorityClass>,
    original_affinity: Option<u64>,
}

/// Process-level changes an apply produced (priority boost, background lower,
/// affinity). The command layer wraps this with any power-profile result.
#[derive(Default, Clone)]
pub struct ApplyOutcome {
    pub boosted: Vec<PriorityChange>,
    pub lowered: Vec<PriorityChange>,
    pub affinity: Vec<AffinityChange>,
}

/// Shared watcher state. Holds its own read-only DB connection (WAL allows a
/// second connection in-process) and the set of currently-boosted games.
pub struct GameWatcher {
    db: Database,
    active: Mutex<HashMap<i64, Vec<WatchedProcess>>>,
}

impl GameWatcher {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            active: Mutex::new(HashMap::new()),
        }
    }

    /// Start the background polling thread and return a shareable handle.
    pub fn spawn(db: Database) -> Arc<Self> {
        let watcher = Arc::new(Self::new(db));
        let w = watcher.clone();
        std::thread::spawn(move || loop {
            w.tick();
            std::thread::sleep(Duration::from_secs(2));
        });
        watcher
    }

    /// Whether a game is currently boosted by the watcher.
    pub fn is_active(&self, game_id: i64) -> bool {
        self.active
            .lock()
            .map(|m| m.contains_key(&game_id))
            .unwrap_or(false)
    }

    /// Apply a game's profile to its running processes (idempotent: a game
    /// already boosted is left untouched so restore tracking isn't clobbered).
    pub fn apply_game(&self, game: &Game, profile: &GameProfile) -> ApplyOutcome {
        {
            let Ok(active) = self.active.lock() else {
                return ApplyOutcome::default();
            };
            if active.contains_key(&game.id) {
                return ApplyOutcome::default();
            }
        }

        let processes = games::running_process_names();
        let pids = games::running_pids(&game.exe_name, &processes);
        let mut outcome = ApplyOutcome::default();
        if pids.is_empty() {
            return outcome;
        }

        let target = games::priority_class(&profile.cpu_priority);
        let mask = games::parse_affinity(profile.affinity_mask.as_deref());
        let background: Vec<u32> = if profile.cleanup_bg {
            processes
                .iter()
                .filter(|(name, pid)| {
                    !pids.contains(pid)
                        && *pid != std::process::id()
                        && classify(name) == ProcessClass::Safe
                })
                .map(|(_, pid)| *pid)
                .collect()
        } else {
            Vec::new()
        };

        let mut watched: Vec<WatchedProcess> = Vec::new();

        for pid in pids {
            let name = name_of(&processes, pid);
            if let Some(cur) = win::process::get_priority(pid) {
                if cur != target {
                    match win::process::set_priority(pid, target) {
                        Ok(()) => {
                            outcome.boosted.push(PriorityChange {
                                pid,
                                name: name.clone(),
                                from: cur.as_str().into(),
                                to: target.as_str().into(),
                            });
                            watched.push(WatchedProcess {
                                pid,
                                original_priority: Some(cur),
                                original_affinity: None,
                            });
                        }
                        Err(e) => crate::logging::warn(&format!(
                            "boost priority for pid {pid} failed: {e}"
                        )),
                    }
                }
            }
            if let Some(mask) = mask {
                let orig = win::process::get_affinity(pid);
                match win::process::set_affinity(pid, mask) {
                    Ok(()) => {
                        outcome.affinity.push(AffinityChange {
                            pid,
                            name: name.clone(),
                            from: orig,
                            to: mask,
                        });
                        if let Some(w) = watched.iter_mut().find(|w| w.pid == pid) {
                            w.original_affinity = orig;
                        } else {
                            watched.push(WatchedProcess {
                                pid,
                                original_priority: None,
                                original_affinity: orig,
                            });
                        }
                    }
                    Err(e) => crate::logging::warn(&format!(
                        "set affinity for pid {pid} failed: {e}"
                    )),
                }
            }
        }

        for pid in background {
            if let Some(cur) = win::process::get_priority(pid) {
                if cur != PriorityClass::BelowNormal && cur != PriorityClass::Idle {
                    if let Err(e) = win::process::set_priority(pid, PriorityClass::BelowNormal) {
                        crate::logging::warn(&format!(
                            "lower priority for background pid {pid} failed: {e}"
                        ));
                        continue;
                    }
                    let name = name_of(&processes, pid);
                    outcome.lowered.push(PriorityChange {
                        pid,
                        name,
                        from: cur.as_str().into(),
                        to: "below_normal".into(),
                    });
                    watched.push(WatchedProcess {
                        pid,
                        original_priority: Some(cur),
                        original_affinity: None,
                    });
                }
            }
        }

        if let Ok(mut active) = self.active.lock() {
            active.insert(game.id, watched);
        }
        outcome
    }

    /// Force-restore a game's tracked processes. Returns the number restored.
    pub fn restore_game(&self, game_id: i64) -> usize {
        let watched = match self.active.lock() {
            Ok(mut m) => m.remove(&game_id),
            Err(_) => None,
        };
        match watched {
            Some(w) => restore_processes(w),
            None => 0,
        }
    }

    /// One polling pass: apply newly-running enabled games, restore exited ones.
    fn tick(&self) {
        let Ok(rows) = self.db.list_games() else {
            return;
        };
        let processes = games::running_process_names();

        let mut to_apply: Vec<(Game, GameProfile)> = Vec::new();
        let mut to_restore: Vec<Vec<WatchedProcess>> = Vec::new();

        {
            let Ok(mut active) = self.active.lock() else {
                return;
            };
            for row in rows {
                let Some(profile) = self.db.get_game_profile(row.id).ok().flatten() else {
                    continue;
                };
                if !profile.enabled {
                    if let Some(w) = active.remove(&row.id) {
                        to_restore.push(w);
                    }
                    continue;
                }
                let game = games::row_to_game(&row);
                let pids = games::running_pids(&game.exe_name, &processes);
                let currently = active.contains_key(&row.id);
                if !pids.is_empty() && !currently {
                    to_apply.push((game, profile));
                } else if pids.is_empty() && currently {
                    if let Some(w) = active.remove(&row.id) {
                        to_restore.push(w);
                    }
                }
            }
        }

        for (game, profile) in to_apply {
            self.apply_game(&game, &profile);
        }
        for watched in to_restore {
            restore_processes(watched);
        }
    }
}

fn name_of(processes: &[(String, u32)], pid: u32) -> String {
    processes
        .iter()
        .find(|(_, p)| *p == pid)
        .map(|(n, _)| n.clone())
        .unwrap_or_default()
}

fn restore_processes(watched: Vec<WatchedProcess>) -> usize {
    let mut restored = 0;
    for w in watched {
        if let Some(orig) = w.original_priority {
            if win::process::set_priority(w.pid, orig).is_ok() {
                restored += 1;
            }
        }
        if let Some(orig) = w.original_affinity {
            if win::process::set_affinity(w.pid, orig).is_ok() {
                restored += 1;
            }
        }
    }
    restored
}
