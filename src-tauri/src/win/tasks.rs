//! Scheduled-task enumeration (Phase 6) via `schtasks /query /fo CSV /v`.
//! The Task Scheduler COM API is heavier; schtasks is available on every
//! Windows edition and its CSV output parses cleanly.

use crate::models::services::ScheduledTask;

/// Parse one line of `schtasks /query /fo CSV /v` (header-aware). Returns
/// `None` for blank/separator lines.
///
/// Columns (all present in verbose CSV): TaskName, Next Run Time, Status,
/// Logon Mode, Last Run Time, Last Result, Author, Task To Run, Start In,
/// Comment, Scheduled Task State, Idle Time, Power Management, Run As User,
/// Schedule Type, Start Time, Start Date, End Date, Days, Months, Repeat,
/// Repeat: Duration, Repeat: Stop at Duration End, Repeat: Stop at Duration
/// End, Repeat: Stop If Runs Longer Than, Repeat: Interval.
pub fn parse_scheduled_task_csv_line(line: &str, headers: &[String]) -> Option<ScheduledTask> {
    let line = line.trim();
    if line.is_empty() || line == "\"TaskName\"" {
        return None;
    }
    let fields = split_csv(line);
    let col = |name: &str| -> Option<&str> {
        headers.iter().position(|h| h.eq_ignore_ascii_case(name)).and_then(|i| fields.get(i).map(|s| s.as_str()))
    };
    let task_name = col("TaskName").unwrap_or_default().trim_matches('"').to_string();
    if task_name.is_empty() {
        return None;
    }
    Some(ScheduledTask {
        name: task_name,
        status: col("Status").unwrap_or_default().trim_matches('"').to_string(),
        next_run: col("Next Run Time").unwrap_or_default().trim_matches('"').to_string(),
        last_run: col("Last Run Time").unwrap_or_default().trim_matches('"').to_string(),
        author: col("Author").unwrap_or_default().trim_matches('"').to_string(),
        action: col("Task To Run").unwrap_or_default().trim_matches('"').to_string(),
        run_as: col("Run As User").unwrap_or_default().trim_matches('"').to_string(),
        signature: "unavailable".to_string(), // enriched by engine::services
    })
}

/// Split a CSV line, honoring quoted fields (schtasks always quotes with `"`).
pub fn split_csv(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if in_quotes && chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = !in_quotes;
                }
            }
            ',' if !in_quotes => {
                out.push(current.clone());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    out.push(current);
    out
}

/// Enumerate scheduled tasks on Windows (empty elsewhere).
#[cfg(windows)]
pub fn list_scheduled_tasks() -> Vec<ScheduledTask> {
    use std::process::Command;

    let output = match Command::new("schtasks.exe")
        .args(["/query", "/fo", "CSV", "/v"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_scheduled_task_csv(&text)
}

#[cfg(not(windows))]
pub fn list_scheduled_tasks() -> Vec<ScheduledTask> {
    Vec::new()
}

/// Parse the full `schtasks /query /fo CSV /v` document.
pub fn parse_scheduled_task_csv(text: &str) -> Vec<ScheduledTask> {
    let mut lines = text.lines();
    let Some(header_line) = lines.next() else {
        return Vec::new();
    };
    let mut headers: Vec<String> = split_csv(header_line)
        .into_iter()
        .map(|s| s.trim_matches('"').to_string())
        .collect();
    // Some locales add a byte-order mark; strip it from the first header.
    if let Some(first) = headers.first_mut() {
        *first = first.trim_start_matches('\u{feff}').to_string();
    }

    // The row count and footer lines are not tasks.
    let mut out = Vec::new();
    for line in lines {
        let l = line.trim();
        // schtasks quotes every field, so the "Total tasks: N" footer arrives
        // as `"Total tasks: N"` — strip quotes before checking.
        let unquoted = l.trim_matches('"').trim();
        if l.is_empty()
            || unquoted.to_ascii_lowercase().starts_with("total tasks")
            || unquoted.starts_with("info:")
            || l.starts_with('"')
                && l.len() > 0
                && l[1..].trim_start_matches('"').trim_start().is_empty()
        {
            continue;
        }
        if let Some(task) = parse_scheduled_task_csv_line(l, &headers) {
            // Skip the "Info:" / blank trailing rows schtasks adds.
            if task.name.is_empty() {
                continue;
            }
            out.push(task);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_csv_line() {
        let line = r#""\MyTask","11/20/2026 3:00:00 AM","Ready","Interactive only","Never","0","","C:\tools\x.exe","","","Ready","","","SYSTEM","Once","","","","","","","","","""#;
        let headers = [
            "TaskName",
            "Next Run Time",
            "Status",
            "Logon Mode",
            "Last Run Time",
            "Last Result",
            "Author",
            "Task To Run",
            "Start In",
            "Comment",
            "Scheduled Task State",
            "Idle Time",
            "Power Management",
            "Run As User",
            "Schedule Type",
            "Start Time",
            "Start Date",
            "End Date",
            "Days",
            "Months",
            "Repeat",
            "Repeat: Duration",
            "Repeat: Stop at Duration End",
            "Repeat: Stop If Runs Longer Than",
            "Repeat: Interval",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
        let t = parse_scheduled_task_csv_line(line, &headers).expect("parses");
        assert_eq!(t.name, r"\MyTask");
        assert_eq!(t.status, "Ready");
        assert_eq!(t.author, "");
        assert_eq!(t.action, r"C:\tools\x.exe");
        assert_eq!(t.run_as, "SYSTEM");
        assert_eq!(t.next_run, "11/20/2026 3:00:00 AM");
    }

    #[test]
    fn ignores_totals_and_blank_lines() {
        let csv = "\"TaskName\",\"Status\"\n\"\\T1\",\"Ready\"\n\n\"Total tasks: 1\"\n";
        let tasks = parse_scheduled_task_csv(csv);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, r"\T1");
    }
}