use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::provider::tailscale::TailscaleProvider;
use crate::provider::warp::WarpProvider;
use crate::provider::wireguard::WireGuardProvider;
use crate::provider::{ProviderInfo, VpnError, VpnProvider, VpnStatus};
use crate::settings::AppSettings;

pub struct AppState {
    pub providers: Vec<Arc<Mutex<Box<dyn VpnProvider>>>>,
    pub settings: Arc<Mutex<AppSettings>>,
    pub settings_path: PathBuf,
}

impl AppState {
    pub fn new(settings_path: PathBuf) -> Self {
        let settings = AppSettings::load(&settings_path);

        let providers: Vec<Arc<Mutex<Box<dyn VpnProvider>>>> = vec![
            Arc::new(Mutex::new(Box::new(TailscaleProvider::new()))),
            Arc::new(Mutex::new(Box::new(WarpProvider::new()))),
            Arc::new(Mutex::new(Box::new(WireGuardProvider::new()))),
        ];

        Self {
            providers,
            settings: Arc::new(Mutex::new(settings)),
            settings_path,
        }
    }

    pub async fn status_all(&self) -> Vec<Result<VpnStatus, VpnError>> {
        let mut results = Vec::new();
        for provider in &self.providers {
            let p = provider.lock().await;
            if !p.is_installed() {
                results.push(Err(VpnError::NotInstalled));
                continue;
            }
            results.push(p.status().await);
        }
        results
    }

    pub async fn list_providers(&self) -> Vec<ProviderInfo> {
        let settings = self.settings.lock().await;
        let mut infos = Vec::new();
        for provider in &self.providers {
            let p = provider.lock().await;
            let name = p.name().to_string();
            infos.push(ProviderInfo {
                installed: p.is_installed(),
                enabled: settings.is_provider_visible(&name),
                name,
            });
        }
        infos
    }

    pub async fn find_provider(&self, name: &str) -> Option<Arc<Mutex<Box<dyn VpnProvider>>>> {
        for provider in &self.providers {
            let p = provider.lock().await;
            if p.name().eq_ignore_ascii_case(name) {
                drop(p);
                return Some(Arc::clone(provider));
            }
        }
        None
    }

    pub async fn save_settings(&self) -> Result<(), String> {
        let settings = self.settings.lock().await;
        settings.save(&self.settings_path)
    }
}
