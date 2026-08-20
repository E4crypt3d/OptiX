use tauri::State;

use crate::db::sqlite::Database;
use crate::engine::services;
use crate::error::Result;
use crate::models::services::{
    ScheduledTask, ServiceActionResult, ServiceInfo, StartupActionResult, StartupEntry,
    WSearchStatus,
};

/// Enumerate services with classification applied.
#[tauri::command]
pub async fn list_services() -> Result<Vec<ServiceInfo>> {
    tauri::async_runtime::spawn_blocking(services::list_services)
        .await
        .map_err(|e| crate::error::OptixError::Other(e.to_string()))
}

/// Stop a running service (snapshot-first, reversible).
#[tauri::command]
pub fn stop_service(db: State<'_, Database>, name: String) -> Result<ServiceActionResult> {
    services::stop_service(db.inner(), &name)
}

/// Start a stopped service (snapshot-first, reversible).
#[tauri::command]
pub fn start_service(db: State<'_, Database>, name: String) -> Result<ServiceActionResult> {
    services::start_service(db.inner(), &name)
}

/// Change a service's start type (`auto` | `manual` | `disabled`).
#[tauri::command]
pub fn set_service_start_type(
    db: State<'_, Database>,
    name: String,
    start_type: String,
) -> Result<ServiceActionResult> {
    services::set_start_type(db.inner(), &name, &start_type)
}

/// Current Windows Search state.
#[tauri::command]
pub fn get_wsearch() -> WSearchStatus {
    services::wsearch_status()
}

/// Enable or disable Windows Search.
#[tauri::command]
pub fn set_wsearch(db: State<'_, Database>, enabled: bool) -> Result<ServiceActionResult> {
    services::set_wsearch(db.inner(), enabled)
}

/// Enumerate scheduled tasks with Authenticode signature verification.
#[tauri::command]
pub async fn list_scheduled_tasks() -> Result<Vec<ScheduledTask>> {
    tauri::async_runtime::spawn_blocking(services::list_scheduled_tasks)
        .await
        .map_err(|e| crate::error::OptixError::Other(e.to_string()))
}

/// Enumerate startup applications.
#[tauri::command]
pub fn list_startup() -> Vec<StartupEntry> {
    services::list_startup()
}

/// Enable or disable a registry startup entry (snapshot-first, reversible).
#[tauri::command]
pub fn set_startup_enabled(
    db: State<'_, Database>,
    location: String,
    enabled: bool,
    command: String,
) -> Result<StartupActionResult> {
    services::set_startup_enabled(db.inner(), &location, enabled, &command)
}
