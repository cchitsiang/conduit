use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

use crate::provider::wireguard::WireGuardProvider;
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

    let mut p = provider_arc.lock().await;

    // For WireGuard, update the active interface before calling set_config
    if let ProviderConfig::WireGuard { ref interface, .. } = config {
        if let Some(wg) = p.as_any_mut().downcast_mut::<WireGuardProvider>() {
            wg.interface = interface.clone();
        }
    }

    p.set_config(config).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_wireguard_configs() -> Result<Vec<WgConfigInfo>, String> {
    let configs = WireGuardProvider::list_config_files();
    Ok(configs
        .into_iter()
        .map(|path| {
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            WgConfigInfo {
                name,
                path: path.to_string_lossy().to_string(),
            }
        })
        .collect())
}

#[derive(serde::Serialize)]
pub struct WgConfigInfo {
    pub name: String,
    pub path: String,
}

#[tauri::command]
pub async fn get_wireguard_config_dir() -> Result<String, String> {
    let dir = WireGuardProvider::ensure_user_config_dir()?;
    Ok(dir.to_string_lossy().to_string())
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
