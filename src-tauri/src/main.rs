// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use tokio::sync::Mutex;

fn main() {
    let settings_path = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("com.conduit.app")
        .join("settings.json");

    let app_state = Arc::new(Mutex::new(conduit_lib::state::AppState::new(settings_path)));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            conduit_lib::commands::vpn_connect,
            conduit_lib::commands::vpn_disconnect,
            conduit_lib::commands::vpn_status,
            conduit_lib::commands::vpn_status_all,
            conduit_lib::commands::vpn_get_config,
            conduit_lib::commands::vpn_set_config,
            conduit_lib::commands::vpn_list_providers,
            conduit_lib::commands::get_settings,
            conduit_lib::commands::update_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
