//! Scheduled-task enumeration (Linux): systemd timers + cron.
//!
//! - systemd timers: `systemctl list-timers --all` (system + user scope) —
//!   the modern, structured equivalent of Windows Task Scheduler.
//! - cron: the user crontab (`crontab -l`) and system crontabs
//!   (`/etc/crontab`, `/etc/cron.d/*`, `/etc/cron.{hourly,daily,weekly,monthly}/*`).
//!
//! Timers are the primary source; cron entries are appended with a `cron:`
//! prefix so they're visually distinct. There is no Authenticode equivalent
//! on Linux, so `signature` is always \"unavailable\".

#[cfg(target_os = "linux")]
use std::process::Command;

use crate::models::services::ScheduledTask;

/// `systemctl list-timers --all --no-legend --plain`: columns are
/// `NEXT LEFT LAST PASSED UNIT ACTIVATES`, but LEFT/PASSED are
/// human-readable durations of variable width (and can be `-`), so
/// whitespace column parsing is fragile. Instead, anchor on the right: the
/// last two tokens are always `UNIT ACTIVATES` (aligned column output).
/// NEXT/LAST are the first/second ISO-style dates (`Sat 2026-08-22
/// 11:30:00 PKT`), located by regex — locale-independence is best-effort
/// here since these are display-only strings.
#[cfg(target_os = "linux")]
fn parse_timers(text: &str) -> Vec<ScheduledTask> {
    let mut out = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 2 {
            continue;
        }
        let (unit, activates) = (fields[fields.len() - 2], fields[fields.len() - 1]);
        if !unit.ends_with(".timer") {
            continue;
        }
        let dates = find_dates(line);
        let next_run = dates.first().cloned().unwrap_or_default();
        let last_run = dates.get(1).cloned().unwrap_or_default();
        out.push(ScheduledTask {
            name: unit.to_string(),
            status: "timer".to_string(),
            next_run,
            last_run,
            author: String::new(),
            action: activates.to_string(),
            run_as: String::new(),
            signature: "unavailable".to_string(),
        });
    }
    out
}

/// Find `YYYY-MM-DD HH:MM:SS` timestamps in a line without a regex dependency.
#[cfg(target_os = "linux")]
fn find_dates(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 19 <= bytes.len() {
        // Match `YYYY-MM-DD` then a space then `HH:MM:SS`.
        let date = &bytes[i..i + 10];
        if date.len() == 10
            && date[4] == b'-'
            && date[7] == b'-'
            && date[..4].iter().all(u8::is_ascii_digit)
            && date[5..7].iter().all(u8::is_ascii_digit)
            && date[8..10].iter().all(u8::is_ascii_digit)
        {
            if let Some(rest) = line.get(i + 10..) {
                let rest = rest.trim_start();
                if rest.len() >= 8
                    && rest.as_bytes()[2] == b':'
                    && rest.as_bytes()[5] == b':'
                    && rest[..2].chars().all(|c| c.is_ascii_digit())
                    && rest[3..5].chars().all(|c| c.is_ascii_digit())
                    && rest[6..8].chars().all(|c| c.is_ascii_digit())
                {
                    out.push(format!("{} {}", &line[i..i + 10], &rest[..8]));
                    i += 10 + rest.len();
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

/// Parse the user crontab (`crontab -l` output). Lines are
/// `min hour dom mon dow command` — env assignments and comments are skipped.
#[cfg(target_os = "linux")]
fn parse_crontab(text: &str) -> Vec<ScheduledTask> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.contains('=') {
            continue;
        }
        // Five time fields then the command.
        let mut fields = line.split_whitespace();
        let mut time = Vec::new();
        for _ in 0..5 {
            match fields.next() {
                Some(f) => time.push(f),
                None => break,
            }
        }
        if time.len() < 5 {
            continue;
        }
        let command = fields.collect::<Vec<_>>().join(" ");
        if command.is_empty() {
            continue;
        }
        let schedule = time.join(" ");
        out.push(ScheduledTask {
            name: format!("cron: {}", command.split_whitespace().next().unwrap_or(&command)),
            status: "cron".to_string(),
            next_run: schedule,
            last_run: String::new(),
            author: String::new(),
            action: command,
            run_as: String::new(),
            signature: "unavailable".to_string(),
        });
    }
    out
}

#[cfg(target_os = "linux")]
fn systemctl(scope: Option<&str>, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("systemctl");
    if let Some(scope) = scope {
        cmd.arg(scope);
    }
    cmd.args(args).arg("--no-legend").arg("--no-pager").arg("--plain");
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Enumerate scheduled tasks: systemd timers (system + user) then cron.
#[cfg(target_os = "linux")]
pub fn list_scheduled_tasks() -> Vec<ScheduledTask> {
    let mut out = Vec::new();

    if let Some(text) = systemctl(None, &["list-timers", "--all"]) {
        out.extend(parse_timers(&text));
    }
    if let Some(text) = systemctl(Some("--user"), &["list-timers", "--all"]) {
        out.extend(parse_timers(&text));
    }

    // User crontab (crontab -l exits 1 when there is no crontab — that's fine).
    if let Ok(output) = Command::new("crontab").arg("-l").output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            out.extend(parse_crontab(&text));
        }
    }

    // System cron: /etc/crontab + /etc/cron.d/* (cron lines with a user field)
    // and the run-parts script dirs.
    for path in ["/etc/crontab"] {
        if let Ok(text) = std::fs::read_to_string(path) {
            out.extend(parse_crontab_system(&text));
        }
    }
    if let Ok(entries) = std::fs::read_dir("/etc/cron.d") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e != "d").unwrap_or(true) {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                out.extend(parse_crontab_system(&text));
            }
        }
    }
    for dir in ["/etc/cron.hourly", "/etc/cron.daily", "/etc/cron.weekly", "/etc/cron.monthly"] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    continue;
                }
                out.push(ScheduledTask {
                    name: format!("cron: {dir}/{name}"),
                    status: "cron".to_string(),
                    next_run: dir.rsplit('/').next().unwrap_or("").to_string(),
                    last_run: String::new(),
                    author: String::new(),
                    action: format!("{dir}/{name}"),
                    run_as: "root".to_string(),
                    signature: "unavailable".to_string(),
                });
            }
        }
    }

    out
}

/// Parse system crontabs (`/etc/crontab`, `/etc/cron.d/*`): six time fields
/// (the fifth is a username) then the command.
#[cfg(target_os = "linux")]
fn parse_crontab_system(text: &str) -> Vec<ScheduledTask> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.contains('=') {
            continue;
        }
        let mut fields = line.split_whitespace();
        let mut time = Vec::new();
        for _ in 0..6 {
            match fields.next() {
                Some(f) => time.push(f),
                None => break,
            }
        }
        if time.len() < 6 {
            continue;
        }
        let user = time[5];
        let schedule = time[..5].join(" ");
        let command = fields.collect::<Vec<_>>().join(" ");
        if command.is_empty() {
            continue;
        }
        out.push(ScheduledTask {
            name: format!("cron: {}", command.split_whitespace().next().unwrap_or(&command)),
            status: "cron".to_string(),
            next_run: schedule,
            last_run: String::new(),
            author: String::new(),
            action: command,
            run_as: user.to_string(),
            signature: "unavailable".to_string(),
        });
    }
    out
}

#[cfg(not(target_os = "linux"))]
pub fn list_scheduled_tasks() -> Vec<ScheduledTask> {
    Vec::new()
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn parses_timer_list() {
        let text = "Sat 2026-08-22 11:30:00 PKT 6min Sat 2026-08-22 11:20:00 PKT 3min 0s ago sysstat-collect.timer sysstat-collect.service\n\
                    Sat 2026-08-22 15:27:08 PKT 4h 4min Fri 2026-08-21 19:29:32 PKT - motd-news.timer motd-news.service\n";
        let timers = parse_timers(text);
        assert_eq!(timers.len(), 2);
        assert_eq!(timers[0].name, "sysstat-collect.timer");
        assert_eq!(timers[0].action, "sysstat-collect.service");
        assert!(timers[0].next_run.contains("11:30:00"));
        assert_eq!(timers[1].last_run, "");
    }

    #[test]
    fn parses_user_crontab() {
        let text = "# comment\nMAILTO=\"\"\n*/5 * * * * /usr/bin/foo --bar\n0 2 * * * /opt/backup.sh >/dev/null 2>&1\n";
        let tasks = parse_crontab(text);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].action, "/usr/bin/foo --bar");
        assert_eq!(tasks[0].next_run, "*/5 * * * *");
        assert_eq!(tasks[1].action, "/opt/backup.sh >/dev/null 2>&1");
    }

    #[test]
    fn parses_system_crontab() {
        let text = "17 * * * * root cd / && run-parts --report /etc/cron.hourly\n25 6 * * * root test -x /usr/sbin/anacron || ( cd / && run-parts --report /etc/cron.daily )\n";
        let tasks = parse_crontab_system(text);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].run_as, "root");
        assert_eq!(tasks[0].next_run, "17 * * * *");
        assert!(tasks[0].action.contains("run-parts"));
    }
}
