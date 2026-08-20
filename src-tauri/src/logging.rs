//! Backend logging: every line goes to **both** the dev console (stderr) and
//! `logs.txt` next to the installed Optix executable (falling back to the app
//! data dir when the install dir is not writable).
//!
//! Errors are never silently dropped: best-effort operations log their
//! failures here (`log::error("context", &err)`) while still returning a
//! friendly outcome to the UI. The console line keeps the full detail for
//! developers; the UI keeps its short, user-facing message.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static LOG_LOCK: Mutex<()> = Mutex::new(());

/// Resolve and open `logs.txt` for appending. Called once at startup; also
/// seeds the file with a header so it is easy to find in a fresh install.
pub fn init() {
    let path = resolve_log_path();
    let _ = LOG_PATH.set(path);
    if let Some(p) = LOG_PATH.get() {
        // Create the file eagerly so the path exists even before the first error.
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(p) {
            let _ = writeln!(
                f,
                "[{}] INFO optix v{} logging initialized",
                now_string(),
                env!("CARGO_PKG_VERSION")
            );
        }
    }
}

/// The active `logs.txt` path (for the Settings page).
pub fn log_path() -> Option<&'static Path> {
    LOG_PATH.get().map(|p| p.as_path())
}

/// Log an informational message.
pub fn info(msg: &str) {
    write_line("INFO", msg, "");
}

/// Log a warning (recoverable, but the user should know).
pub fn warn(msg: &str) {
    write_line("WARN", msg, "");
}

/// Log an error with context. This is what best-effort call sites use instead
/// of dropping the error: the full detail lands in the console + logs.txt
/// while the app keeps running with a graceful result.
pub fn error(context: &str, err: &dyn std::fmt::Display) {
    write_line("ERROR", context, &err.to_string());
}

/// Format a panic for the log (installed as the panic hook in `run()`).
pub fn panic(info: &std::panic::PanicHookInfo<'_>) {
    write_line("PANIC", "unhandled panic", &info.to_string());
}

fn resolve_log_path() -> PathBuf {
    // The software installation directory, as requested — writable here since
    // Optix runs elevated (requireAdministrator). Fall back to the app data
    // dir when that fails (e.g. Linux dev builds).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("logs.txt");
            if writable(&candidate) {
                return candidate;
            }
        }
    }
    let fallback = crate::db::sqlite::data_dir().join("logs.txt");
    let _ = std::fs::create_dir_all(fallback.parent().unwrap_or(Path::new(".")));
    fallback
}

fn writable(path: &Path) -> bool {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .is_ok()
}

fn write_line(level: &str, context: &str, detail: &str) {
    let line = format!("[{}] {level} {context} {detail}", now_string());
    // Console: devs always see errors during development and in terminal runs.
    eprintln!("[optix] {line}");
    if let Some(path) = LOG_PATH.get() {
        let _guard = LOG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// `YYYY-MM-DD HH:MM:SS` (UTC) from the current time, without extra crates.
fn now_string() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400);
    let (h, m, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);

    // Howard Hinnant's civil-from-days algorithm (same as engine::crash).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_is_readable() {
        let s = now_string();
        assert_eq!(s.len(), 19);
        assert!(s.chars().nth(4) == Some('-') && s.chars().nth(10) == Some(' '));
    }

    #[test]
    fn path_resolves_somewhere() {
        // init() must always find a writable-ish target, even on dev hosts.
        init();
        assert!(log_path().is_some());
    }
}
