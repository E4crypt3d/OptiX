mod commands;
mod db;
mod engine;
mod error;
mod models;
pub mod win;

use commands::system::MonitorState;
use db::sqlite::Database;
use engine::optimizer::OptimizerState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Opens (and migrates) the SQLite database at startup.
            let db = Database::open()?;
            app.manage(db);
            app.manage(MonitorState::new());
            app.manage(OptimizerState::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::scan_system,
            commands::system::system_stats,
            commands::system::record_sample,
            commands::system::recent_samples,
            commands::snapshot::create_snapshot,
            commands::snapshot::list_snapshots,
            commands::snapshot::list_changes,
            commands::snapshot::delete_snapshot,
            commands::snapshot::restore_snapshot,
            commands::snapshot::diff_snapshots,
            commands::cleanup::scan_cleanup,
            commands::cleanup::run_cleanup,
            commands::processes::list_processes,
            commands::processes::kill_process,
            commands::processes::set_process_priority,
            commands::processes::apply_gaming_mode,
            commands::processes::restore_gaming_mode,
            commands::power::list_power_schemes,
            commands::power::list_power_profiles,
            commands::power::apply_power_profile,
            commands::power::list_nic_adapters,
            commands::power::disable_nic_power_saving,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
