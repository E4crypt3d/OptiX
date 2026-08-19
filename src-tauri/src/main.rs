// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // v1 privilege model: run as a single elevated process. On Windows, if we
    // are not already elevated, relaunch through UAC and exit this instance.
    #[cfg(windows)]
    {
        if !optix_lib::win::elevation::ensure_elevated() {
            return;
        }
    }

    optix_lib::run()
}
