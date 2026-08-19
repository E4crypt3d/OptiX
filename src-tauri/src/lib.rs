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
            // Game-mode watcher runs on its own read-only DB connection and
            // auto-applies/restores enabled game profiles on launch/exit.
            let watcher = engine::game_watcher::GameWatcher::spawn(Database::open()?);
            app.manage(watcher);
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
            commands::bloatware::scan_bloatware,
            commands::bloatware::remove_bloatware,
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
            commands::services::list_services,
            commands::services::stop_service,
            commands::services::start_service,
            commands::services::set_service_start_type,
            commands::services::get_wsearch,
            commands::services::set_wsearch,
            commands::services::list_startup,
            commands::services::set_startup_enabled,
            commands::network::network_status,
            commands::network::list_dns_servers,
            commands::network::benchmark_dns,
            commands::network::apply_dns,
            commands::network::tcp_parameters,
            commands::gpu::list_gpu_adapters,
            commands::gpu::list_gpu_toggles,
            commands::gpu::set_gpu_toggle,
            commands::gpu::scan_shader_caches,
            commands::gpu::clear_shader_caches,
            commands::gpu::get_amd_shader_cache,
            commands::gpu::set_amd_shader_cache,
            commands::games::detect_games,
            commands::games::list_games,
            commands::games::add_game,
            commands::games::add_manual_game,
            commands::games::remove_game,
            commands::games::get_game_profile,
            commands::games::save_game_profile,
            commands::games::apply_game_profile,
            commands::games::restore_game_profile,
            commands::benchmark::run_fps_benchmark,
            commands::benchmark::run_stress_benchmark,
            commands::benchmark::list_benchmarks,
            commands::benchmark::delete_benchmark,
            commands::benchmark::benchmark_frame_times,
            commands::crash::scan_crashes,
            commands::crash::generate_crash_report,
            commands::diagnostics::run_diagnostics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
