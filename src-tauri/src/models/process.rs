use serde::Serialize;

/// Safety classification for a running process.
///
/// `Required` processes must never be killed or deprioritized by Optix;
/// `Safe` are user apps in the allowlist; `Unknown` gets shown with flags and
/// defaults to no action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessClass {
    Required,
    Safe,
    Unknown,
}

impl ProcessClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProcessClass::Required => "required",
            ProcessClass::Safe => "safe",
            ProcessClass::Unknown => "unknown",
        }
    }
}

/// Windows process priority class. `Realtime` is detectable (so the UI can
/// show it) but **never settable** — Optix must not apply REALTIME priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorityClass {
    Idle,
    BelowNormal,
    Normal,
    AboveNormal,
    High,
    Realtime,
}

impl PriorityClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            PriorityClass::Idle => "idle",
            PriorityClass::BelowNormal => "below_normal",
            PriorityClass::Normal => "normal",
            PriorityClass::AboveNormal => "above_normal",
            PriorityClass::High => "high",
            PriorityClass::Realtime => "realtime",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "idle" => Some(PriorityClass::Idle),
            "below_normal" => Some(PriorityClass::BelowNormal),
            "normal" => Some(PriorityClass::Normal),
            "above_normal" => Some(PriorityClass::AboveNormal),
            "high" => Some(PriorityClass::High),
            "realtime" => Some(PriorityClass::Realtime),
            _ => None,
        }
    }

    /// Whether Optix is allowed to apply this class. REALTIME is forbidden
    /// (it can freeze input/audio and is never appropriate for gaming).
    pub fn is_settable(&self) -> bool {
        !matches!(self, PriorityClass::Realtime)
    }
}

/// A running process, enriched for the Process & RAM manager.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessDetail {
    pub pid: u32,
    pub name: String,
    pub exe: String,
    pub cpu_usage_percent: f32,
    pub memory_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_written_bytes: u64,
    pub start_time: u64,
    pub parent_pid: Option<u32>,
    /// Human-readable process status (e.g. "running", "sleeping").
    pub status: String,
    /// "required" | "safe" | "unknown".
    pub classification: String,
    /// True when the process is a system/service process (session 0 or a
    /// hard-coded core name) and must not be killed.
    pub is_system: bool,
    /// Current priority class, or `None` when not readable (off Windows).
    pub priority: Option<String>,
}

/// Result of applying gaming mode: every priority change Optix made, kept for
/// restore-on-exit.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamingModeResult {
    pub boosted: Vec<PriorityChange>,
    pub lowered: Vec<PriorityChange>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriorityChange {
    pub pid: u32,
    pub name: String,
    pub from: String,
    pub to: String,
}
