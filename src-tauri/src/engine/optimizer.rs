//! Gaming mode: boost a game process and lower background processes, then
//! restore every changed priority on exit. Pure bookkeeping lives here; the
//! actual `SetPriorityClass` call is Windows-gated in `win::process`.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::models::process::{GamingModeResult, PriorityChange, PriorityClass};
use crate::win;

/// Records a priority change so it can be reverted later.
#[derive(Clone)]
struct AppliedChange {
    pid: u32,
    original: PriorityClass,
    current: PriorityClass,
}

/// Shared gaming-mode state. Tracks every priority Optix changed so a later
/// `restore` call can put processes back to their original class.
pub struct OptimizerState {
    changes: Mutex<HashMap<u32, AppliedChange>>,
}

impl OptimizerState {
    pub fn new() -> Self {
        Self {
            changes: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for OptimizerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether gaming mode can actually change priorities (Windows only).
fn priority_supported() -> bool {
    cfg!(windows)
}

/// Boost `game_pids` to `AboveNormal` and lower `background_pids` to
/// `BelowNormal`, recording originals for restore. No-op (empty result) off
/// Windows.
pub fn apply(
    state: &OptimizerState,
    game_pids: &[u32],
    background_pids: &[u32],
    name_of: impl Fn(u32) -> String,
) -> GamingModeResult {
    if !priority_supported() {
        return GamingModeResult {
            boosted: Vec::new(),
            lowered: Vec::new(),
        };
    }

    let mut boosted = Vec::new();
    let mut lowered = Vec::new();
    let mut changes = match state.changes.lock() {
        Ok(m) => m,
        Err(_) => return GamingModeResult { boosted, lowered },
    };

    let mut set = |pid: u32, target: PriorityClass, out: &mut Vec<PriorityChange>| {
        // Read the *current* class (not necessarily the original) so re-running
        // gaming mode is idempotent.
        let Some(current) = win::process::get_priority(pid) else {
            return;
        };
        if current == target {
            return;
        }
        if win::process::set_priority(pid, target).is_ok() {
            let name = name_of(pid);
            out.push(PriorityChange {
                pid,
                name: name.clone(),
                from: current.as_str().to_string(),
                to: target.as_str().to_string(),
            });
            // Remember the true original only if this pid wasn't already changed.
            let entry = changes.entry(pid).or_insert(AppliedChange {
                pid,
                original: current,
                current: target,
            });
            entry.current = target;
        }
    };

    for &pid in game_pids {
        set(pid, PriorityClass::AboveNormal, &mut boosted);
    }
    for &pid in background_pids {
        set(pid, PriorityClass::BelowNormal, &mut lowered);
    }

    GamingModeResult { boosted, lowered }
}

/// Restore every process whose priority Optix changed back to its original
/// class. Returns the number of processes restored.
pub fn restore(state: &OptimizerState) -> usize {
    if !priority_supported() {
        return 0;
    }
    let changes: Vec<AppliedChange> = match state.changes.lock() {
        Ok(mut m) => m.drain().map(|(_, c)| c).collect(),
        Err(_) => return 0,
    };

    let mut restored = 0;
    for change in changes {
        // Skip if the process already exited (get_priority returns None).
        if win::process::get_priority(change.pid).is_some()
            && win::process::set_priority(change.pid, change.original).is_ok()
        {
            restored += 1;
        }
    }
    restored
}
