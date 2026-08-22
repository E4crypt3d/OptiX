//! Scheduled-task enumeration (Phase 6).
//!
//! Primary: PowerShell `Get-ScheduledTask` — structured, locale-independent
//! JSON output with stable property names.
//!
//! Fallback: `schtasks /query /fo CSV /v` — legacy CSV parser kept for
//! environments where PowerShell is unavailable or blocked.

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

/// PowerShell script that returns scheduled tasks as a JSON array.
/// Uses `Get-ScheduledTask` (structured, locale-independent) and joins
/// `Get-ScheduledTaskInfo` for run-time data.
#[cfg(windows)]
const PS_LIST_TASKS: &str = r#"
$tasks = Get-ScheduledTask -ErrorAction SilentlyContinue
if (-not $tasks) { Write-Output '[]'; exit }
$results = foreach ($t in $tasks) {
    $info = $t | Get-ScheduledTaskInfo -ErrorAction SilentlyContinue
    [PSCustomObject]@{
        # TaskPath + TaskName matches schtasks' full-path format, keeps names
        # unique across folders, and stays consistent with the CSV fallback.
        Name     = $t.TaskPath + $t.TaskName
        Status   = $t.State
        NextRun  = if ($info.NextRunTime -and $info.NextRunTime.Year -gt 1999) { $info.NextRunTime.ToString('g') } else { '' }
        LastRun  = if ($info.LastRunTime -and $info.LastRunTime.Year -gt 1999) { $info.LastRunTime.ToString('g') } else { '' }
        Author   = $t.Author
        Action   = if ($t.Actions.Count -gt 0) { $t.Actions[0].Execute } else { '' }
        RunAs    = $t.Principal.UserId
    }
}
$results | ConvertTo-Json -Compress
"#;

/// Enumerate scheduled tasks on Windows (empty elsewhere).
///
/// Tries PowerShell `Get-ScheduledTask` first (locale-independent JSON).
/// Falls back to `schtasks /query /fo CSV /v` if PowerShell is unavailable.
#[cfg(windows)]
pub fn list_scheduled_tasks() -> Vec<ScheduledTask> {
    if let Some(tasks) = list_scheduled_tasks_powershell() {
        return tasks;
    }
    list_scheduled_tasks_schtasks()
}

/// PowerShell-based enumeration — preferred because output is locale-independent.
#[cfg(windows)]
fn list_scheduled_tasks_powershell() -> Option<Vec<ScheduledTask>> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", PS_LIST_TASKS])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_scheduled_tasks_json(&text)
}

/// Parse the JSON produced by `Get-ScheduledTask | ConvertTo-Json`. The script
/// emits a bare object for a single task and an array for several, so both
/// shapes are normalized.
#[cfg(any(windows, test))]
fn parse_scheduled_tasks_json(text: &str) -> Option<Vec<ScheduledTask>> {
    let text = text.trim();
    if text.is_empty() || text == "[]" {
        return Some(Vec::new());
    }
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let values: Vec<&serde_json::Value> = match &value {
        serde_json::Value::Array(items) => items.iter().collect(),
        serde_json::Value::Object(_) => vec![&value],
        _ => return None,
    };
    let mut out = Vec::with_capacity(values.len());
    for v in values {
        let name = v.get("Name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        out.push(ScheduledTask {
            name,
            status: v.get("Status").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            next_run: v.get("NextRun").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            last_run: v.get("LastRun").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            author: v.get("Author").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            action: v.get("Action").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            run_as: v.get("RunAs").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            signature: "unavailable".to_string(),
        });
    }
    Some(out)
}

/// Fallback: schtasks CSV enumeration (legacy, locale-sensitive).
#[cfg(windows)]
fn list_scheduled_tasks_schtasks() -> Vec<ScheduledTask> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = match Command::new("schtasks.exe")
        .args(["/query", "/fo", "CSV", "/v"])
        .creation_flags(CREATE_NO_WINDOW)
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
                && !l.is_empty()
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

    #[test]
    fn parses_powershell_json_array() {
        let json = r#"[
  {"Name":"\\Microsoft\\Windows\\UpdateOrchestrator\\USO_UxBroker","Status":"Ready","NextRun":"8/21/2026 3:00 AM","LastRun":"","Author":"Microsoft Corporation","Action":"%windir%\\system32\\usocoreworker.exe","RunAs":"SYSTEM"},
  {"Name":"","Status":"Disabled","NextRun":"","LastRun":"","Author":"","Action":"","RunAs":""}
]"#;
        let tasks = parse_scheduled_tasks_json(json).expect("parses array");
        // The row with an empty name is dropped.
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "\\Microsoft\\Windows\\UpdateOrchestrator\\USO_UxBroker");
        assert_eq!(tasks[0].status, "Ready");
        assert_eq!(tasks[0].author, "Microsoft Corporation");
        assert_eq!(tasks[0].action, "%windir%\\system32\\usocoreworker.exe");
        assert_eq!(tasks[0].run_as, "SYSTEM");
        assert_eq!(tasks[0].signature, "unavailable");
    }

    #[test]
    fn parses_single_object_empty_and_garbage() {
        // `ConvertTo-Json` emits a bare object when exactly one task matches.
        let single = r#"{"Name":"\\T1","Status":"Ready","NextRun":"","LastRun":"","Author":"","Action":"","RunAs":""}"#;
        let tasks = parse_scheduled_tasks_json(single).expect("parses single object");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "\\T1");

        assert_eq!(parse_scheduled_tasks_json("").expect("empty").len(), 0);
        assert_eq!(parse_scheduled_tasks_json("[]").expect("empty array").len(), 0);
        assert!(parse_scheduled_tasks_json("not json").is_none());
    }
}