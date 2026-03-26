use async_trait::async_trait;
use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::provider::{
    ConnectOptions, ProviderConfig, VpnError, VpnProvider, VpnStatus,
};
use crate::util::detect::is_tool_installed;
use crate::util::exec::exec_command;

const TOOL_NAME: &str = "wg";
const TIMEOUT: u64 = 10;

const CONFIG_DIRS: &[&str] = &[
    "/etc/wireguard",
    "/opt/homebrew/etc/wireguard",
    "/usr/local/etc/wireguard",
];

pub struct WireGuardProvider {
    pub interface: String,
}

impl WireGuardProvider {
    pub fn new() -> Self {
        Self {
            interface: "wg0".to_string(),
        }
    }

    pub fn with_interface(interface: &str) -> Self {
        Self {
            interface: interface.to_string(),
        }
    }

    fn parse_status(output: &str, interface: &str) -> Result<VpnStatus, VpnError> {
        let mut extra = HashMap::new();
        extra.insert("interface".to_string(), interface.to_string());

        if output.trim().is_empty() {
            return Ok(VpnStatus {
                provider: "WireGuard".to_string(),
                connected: false,
                ip: None,
                since: None,
                latency_ms: None,
                extra,
            });
        }

        let mut has_peer = false;

        for line in output.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("endpoint:") {
                let val = trimmed.trim_start_matches("endpoint:").trim();
                extra.insert("endpoint".to_string(), val.to_string());
                has_peer = true;
            } else if trimmed.starts_with("latest handshake:") {
                let val = trimmed.trim_start_matches("latest handshake:").trim();
                extra.insert("latest_handshake".to_string(), val.to_string());
                has_peer = true;
            } else if trimmed.starts_with("transfer:") {
                let val = trimmed.trim_start_matches("transfer:").trim();
                if let Some((rx, tx)) = val.split_once(',') {
                    extra.insert("transfer_rx".to_string(), rx.trim().to_string());
                    extra.insert("transfer_tx".to_string(), tx.trim().to_string());
                }
                has_peer = true;
            } else if trimmed.starts_with("peer:") {
                has_peer = true;
            }
        }

        Ok(VpnStatus {
            provider: "WireGuard".to_string(),
            connected: has_peer,
            ip: None,
            since: None,
            latency_ms: None,
            extra,
        })
    }

    pub fn list_config_files() -> Vec<PathBuf> {
        let mut configs = Vec::new();
        for dir in CONFIG_DIRS {
            let path = PathBuf::from(dir);
            if path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&path) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.extension().map(|e| e == "conf").unwrap_or(false) {
                            configs.push(p);
                        }
                    }
                }
            }
        }
        configs
    }

    async fn exec_with_sudo(command: &str) -> Result<String, VpnError> {
        let script = format!(
            "do shell script \"{}\" with administrator privileges",
            command.replace('\\', "\\\\").replace('"', "\\\"")
        );
        exec_command("osascript", &["-e", &script], TIMEOUT)
            .await
            .map_err(|e| {
                let err = VpnError::from(e);
                if let VpnError::CliError(ref msg) = err {
                    if msg.contains("User canceled") || msg.contains("(-128)") {
                        return VpnError::PermissionDenied;
                    }
                }
                err
            })
    }
}

#[async_trait]
impl VpnProvider for WireGuardProvider {
    fn name(&self) -> &str {
        "WireGuard"
    }

    fn is_installed(&self) -> bool {
        is_tool_installed(TOOL_NAME)
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    async fn connect(&self, _opts: ConnectOptions) -> Result<(), VpnError> {
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }
        let cmd = format!("wg-quick up {}", self.interface);
        Self::exec_with_sudo(&cmd).await?;
        Ok(())
    }

    async fn disconnect(&self) -> Result<(), VpnError> {
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }
        let cmd = format!("wg-quick down {}", self.interface);
        Self::exec_with_sudo(&cmd).await?;
        Ok(())
    }

    async fn status(&self) -> Result<VpnStatus, VpnError> {
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }
        // Use ifconfig (no sudo) to check if the interface is up.
        // This avoids triggering the macOS admin password prompt on every poll.
        let mut extra = HashMap::new();
        extra.insert("interface".to_string(), self.interface.clone());

        match exec_command("ifconfig", &[&self.interface], TIMEOUT).await {
            Ok(output) => {
                let connected = output.contains("status: active")
                    || (output.contains("UP") && output.contains("RUNNING"));

                // Try to extract the IP address from ifconfig output
                let ip = output
                    .lines()
                    .find(|line| line.trim().starts_with("inet "))
                    .and_then(|line| line.split_whitespace().nth(1))
                    .map(String::from);

                Ok(VpnStatus {
                    provider: "WireGuard".to_string(),
                    connected,
                    ip,
                    since: None,
                    latency_ms: None,
                    extra,
                })
            }
            Err(_) => {
                // Interface doesn't exist — not connected
                Ok(VpnStatus {
                    provider: "WireGuard".to_string(),
                    connected: false,
                    ip: None,
                    since: None,
                    latency_ms: None,
                    extra,
                })
            }
        }
    }

    async fn get_config(&self) -> Result<ProviderConfig, VpnError> {
        let configs = Self::list_config_files();
        let config_file = configs
            .into_iter()
            .find(|p| {
                p.file_stem()
                    .map(|s| s.to_string_lossy() == self.interface)
                    .unwrap_or(false)
            })
            .unwrap_or_else(|| PathBuf::from(format!("/etc/wireguard/{}.conf", self.interface)));

        Ok(ProviderConfig::WireGuard {
            config_file,
            interface: self.interface.clone(),
        })
    }

    async fn set_config(&self, _config: ProviderConfig) -> Result<(), VpnError> {
        // Interface switching is handled by the caller mutating self.interface
        // before calling connect. See commands::vpn_set_config for WireGuard.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_connected() {
        let output = "interface: wg0\n  \
                       public key: abc123=\n  \
                       private key: (hidden)\n  \
                       listening port: 51820\n\n\
                       peer: xyz789=\n  \
                       endpoint: 203.0.113.1:51820\n  \
                       allowed ips: 10.0.0.0/24\n  \
                       latest handshake: 1 minute, 30 seconds ago\n  \
                       transfer: 1.23 MiB received, 4.56 MiB sent";

        let status = WireGuardProvider::parse_status(output, "wg0").unwrap();
        assert!(status.connected);
        assert_eq!(status.provider, "WireGuard");
        assert_eq!(status.extra.get("interface").map(|s| s.as_str()), Some("wg0"));
        assert_eq!(status.extra.get("endpoint").map(|s| s.as_str()), Some("203.0.113.1:51820"));
        assert_eq!(status.extra.get("transfer_rx").map(|s| s.as_str()), Some("1.23 MiB received"));
        assert_eq!(status.extra.get("transfer_tx").map(|s| s.as_str()), Some("4.56 MiB sent"));
        assert_eq!(status.extra.get("latest_handshake").map(|s| s.as_str()), Some("1 minute, 30 seconds ago"));
    }

    #[test]
    fn test_parse_status_no_peers() {
        let output = "interface: wg0\n  \
                       public key: abc123=\n  \
                       private key: (hidden)\n  \
                       listening port: 51820";

        let status = WireGuardProvider::parse_status(output, "wg0").unwrap();
        assert!(!status.connected);
        assert_eq!(status.extra.get("interface").map(|s| s.as_str()), Some("wg0"));
    }

    #[test]
    fn test_parse_status_empty() {
        let status = WireGuardProvider::parse_status("", "wg0").unwrap();
        assert!(!status.connected);
    }

    #[test]
    fn test_list_config_files() {
        let files = WireGuardProvider::list_config_files();
        for file in &files {
            assert!(file.extension().map(|e| e == "conf").unwrap_or(false));
        }
    }
}
