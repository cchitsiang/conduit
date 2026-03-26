// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use tauri::{Emitter, Manager, RunEvent};
use tokio::sync::Mutex;

fn main() {
    let settings_path = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("com.conduit.app")
        .join("settings.json");

    let app_state = Arc::new(Mutex::new(conduit_lib::state::AppState::new(settings_path)));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(app_state.clone())
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
            conduit_lib::commands::list_wireguard_configs,
            conduit_lib::commands::get_wireguard_config_dir,
            conduit_lib::commands::import_wireguard_config,
        ])
        .setup(move |app| {
            // Create system tray
            conduit_lib::tray::create_tray(app.handle())?;

            // Spawn polling loop
            let app_handle = app.handle().clone();
            let poll_state = app_state.clone();

            tauri::async_runtime::spawn(async move {
                loop {
                    let interval = {
                        let state = poll_state.lock().await;
                        let settings = state.settings.lock().await;
                        settings.poll_interval_secs
                    };

                    tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;

                    let state = poll_state.lock().await;
                    let results = state.status_all().await;
                    let statuses: Vec<_> = results.into_iter().filter_map(|r| r.ok()).collect();

                    // Emit status update to frontend
                    let _ = app_handle.emit("vpn-status-changed", &statuses);

                    // Update tray menu
                    let _ = conduit_lib::tray::update_tray_menu(&app_handle, &statuses);
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide instead of close — keep app in menu bar
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let RunEvent::Reopen { .. } = event {
                // macOS: user clicked the Dock icon — show the dashboard
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });
}
