//! WinMac Launcher — run Windows programs and DirectX games on macOS.
//!
//! The crate is split into small modules so the macOS-only Wine plumbing stays
//! isolated from the portable container / config / scanning layer:
//!
//! * [`engine`]     — wine64 detection, downloads and winetricks installation
//! * [`container`]  — bottles on disk, `container.json`, EXE scanning
//! * [`runner`]     — process spawning, environment, log streaming
//! * [`mock`]       — demo backend used on Windows / browser preview builds
//! * [`commands`]   — the Tauri IPC surface consumed by the frontend

mod commands;
mod container;
mod engine;
mod error;
mod mock;
mod models;
mod paths;
mod runner;
mod winetricks;

use engine::EngineRuntime;
use runner::RunnerState;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(RunnerState::default())
        .manage(EngineRuntime::default())
        .setup(|app| {
            // On non-macOS hosts this creates the demo library on first launch.
            mock::seed_if_needed(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_platform_info,
            commands::list_containers,
            commands::get_container,
            commands::create_container,
            commands::update_container,
            commands::delete_container,
            commands::clone_container,
            commands::import_container,
            commands::scan_games,
            commands::set_game_override,
            commands::launch,
            commands::stop_run,
            commands::get_logs,
            commands::clear_logs,
            commands::get_engines,
            commands::add_engine_manual,
            commands::set_selected_engine,
            commands::remove_engine,
            commands::download_engine,
            commands::cancel_engine_download,
            commands::install_winetricks,
            commands::list_winetricks_verbs,
            commands::run_winetricks,
            commands::reveal_in_file_manager,
            commands::open_folder,
            commands::app_version,
            commands::get_running_run,
        ])
        .run(tauri::generate_context!())
        .expect("error while running WinMac Launcher");
}
