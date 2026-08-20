mod commands;
mod db;
mod engine;
mod error;
mod logging;
mod models;
pub mod win;

use commands::system::MonitorState;
use db::sqlite::Database;
use engine::optimizer::OptimizerState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Log everything: devs see errors in the console, and a copy lands in
    // logs.txt next to the installed Optix executable. Panics are captured
    // too, so backend crashes are never invisible.
    logging::init();
    logging::info(&format!("optix v{} starting", env!("CARGO_PKG_VERSION")));
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        logging::panic(info);
        default_hook(info);
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            // Live crash watch: polls the Application event log and emits
            // `optix://crash-detected` when new crashes are logged.
            engine::crash::spawn_crash_watch(app.handle().clone(), 20);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::scan_system,
            commands::system::system_stats,
            commands::system::record_sample,
            commands::system::recent_samples,
            commands::system::app_info,
            commands::snapshot::create_snapshot,
            commands::snapshot::list_snapshots,
            commands::snapshot::list_changes,
            commands::snapshot::delete_snapshot,
            commands::snapshot::restore_snapshot,
            commands::snapshot::diff_snapshots,
            commands::snapshot::create_system_restore_point,
            commands::cleanup::scan_cleanup,
            commands::cleanup::run_cleanup,
            commands::cleanup::dism_component_cleanup,
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
            commands::services::list_scheduled_tasks,
            commands::network::network_status,
            commands::network::benchmark_dns,
            commands::network::apply_dns,
            commands::network::list_tcp_tweaks,
            commands::network::apply_tcp_tweaks,
            commands::network::reset_tcp_tweaks,
            commands::network::ping_test,
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
            commands::games::remove_game_drs_profile,
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
        .map_err(|e| {
            logging::error("tauri run failed", &e);
            e
        })
        .expect("error while running tauri application");
}
