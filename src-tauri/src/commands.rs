use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

use crate::provider::{ConnectOptions, ProviderConfig, ProviderInfo, VpnStatus};
use crate::settings::AppSettings;
use crate::state::AppState;

type AppStateManaged = Arc<Mutex<AppState>>;

#[tauri::command]
pub async fn vpn_connect(
    state: State<'_, AppStateManaged>,
    provider: String,
    opts: Option<ConnectOptions>,
) -> Result<(), String> {
    let state = state.lock().await;
    let provider_arc = state
        .find_provider(&provider)
        .await
        .ok_or_else(|| format!("Provider '{}' not found", provider))?;

    let p = provider_arc.lock().await;
    let connect_opts = opts.unwrap_or(ConnectOptions {
        provider_config: None,
    });
    p.connect(connect_opts)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn vpn_disconnect(
    state: State<'_, AppStateManaged>,
    provider: String,
) -> Result<(), String> {
    let state = state.lock().await;
    let provider_arc = state
        .find_provider(&provider)
        .await
        .ok_or_else(|| format!("Provider '{}' not found", provider))?;

    let p = provider_arc.lock().await;
    p.disconnect().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn vpn_status(
    state: State<'_, AppStateManaged>,
    provider: String,
) -> Result<VpnStatus, String> {
    let state = state.lock().await;
    let provider_arc = state
        .find_provider(&provider)
        .await
        .ok_or_else(|| format!("Provider '{}' not found", provider))?;

    let p = provider_arc.lock().await;
    p.status().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn vpn_status_all(
    state: State<'_, AppStateManaged>,
) -> Result<Vec<VpnStatus>, String> {
    let state = state.lock().await;
    let results = state.status_all().await;
    let mut statuses = Vec::new();
    for result in results {
        if let Ok(status) = result {
            statuses.push(status);
        }
    }
    Ok(statuses)
}

#[tauri::command]
pub async fn vpn_get_config(
    state: State<'_, AppStateManaged>,
    provider: String,
) -> Result<ProviderConfig, String> {
    let state = state.lock().await;
    let provider_arc = state
        .find_provider(&provider)
        .await
        .ok_or_else(|| format!("Provider '{}' not found", provider))?;

    let p = provider_arc.lock().await;
    p.get_config().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn vpn_set_config(
    state: State<'_, AppStateManaged>,
    provider: String,
    config: ProviderConfig,
) -> Result<(), String> {
    let state = state.lock().await;
    let provider_arc = state
        .find_provider(&provider)
        .await
        .ok_or_else(|| format!("Provider '{}' not found", provider))?;

    let p = provider_arc.lock().await;
    p.set_config(config).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn vpn_list_providers(
    state: State<'_, AppStateManaged>,
) -> Result<Vec<ProviderInfo>, String> {
    let state = state.lock().await;
    Ok(state.list_providers().await)
}

#[tauri::command]
pub async fn get_settings(
    state: State<'_, AppStateManaged>,
) -> Result<AppSettings, String> {
    let state = state.lock().await;
    let settings = state.settings.lock().await;
    Ok(settings.clone())
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppStateManaged>,
    settings: AppSettings,
) -> Result<(), String> {
    let state = state.lock().await;
    {
        let mut current = state.settings.lock().await;
        *current = settings;
    }
    state.save_settings().await
}
