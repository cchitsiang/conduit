pub mod tailscale;
pub mod warp;
pub mod wireguard;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::util::exec::ExecError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnStatus {
    pub provider: String,
    pub connected: bool,
    pub ip: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub latency_ms: Option<u32>,
    pub extra: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProviderConfig {
    Tailscale {
        exit_node: Option<String>,
        accept_routes: bool,
        shields_up: bool,
    },
    Warp {
        mode: WarpMode,
        families_mode: bool,
    },
    WireGuard {
        config_file: PathBuf,
        interface: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WarpMode {
    Warp,
    DnsOnly,
    Proxy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectOptions {
    pub provider_config: Option<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub installed: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub enum VpnError {
    NotInstalled,
    CliError(String),
    ParseError(String),
    PermissionDenied,
    Timeout,
}

impl std::fmt::Display for VpnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VpnError::NotInstalled => write!(f, "VPN tool is not installed"),
            VpnError::CliError(e) => write!(f, "CLI error: {}", e),
            VpnError::ParseError(e) => write!(f, "Parse error: {}", e),
            VpnError::PermissionDenied => write!(f, "Permission denied"),
            VpnError::Timeout => write!(f, "Operation timed out"),
        }
    }
}

impl From<ExecError> for VpnError {
    fn from(e: ExecError) -> Self {
        match e {
            ExecError::Timeout => VpnError::Timeout,
            ExecError::IoError(msg) => VpnError::CliError(msg),
            ExecError::NonZeroExit { stderr, .. } => VpnError::CliError(stderr),
        }
    }
}

#[async_trait]
pub trait VpnProvider: Send + Sync {
    fn name(&self) -> &str;
    fn is_installed(&self) -> bool;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    async fn connect(&self, opts: ConnectOptions) -> Result<(), VpnError>;
    async fn disconnect(&self) -> Result<(), VpnError>;
    async fn status(&self) -> Result<VpnStatus, VpnError>;
    async fn get_config(&self) -> Result<ProviderConfig, VpnError>;
    async fn set_config(&self, config: ProviderConfig) -> Result<(), VpnError>;
}
