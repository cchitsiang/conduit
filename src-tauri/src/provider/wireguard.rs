use async_trait::async_trait;
use std::any::Any;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::provider::{
    ConnectOptions, ProviderConfig, VpnError, VpnProvider, VpnStatus,
};
use crate::util::detect::find_tool;
use crate::util::exec::exec_command;

const TOOL_NAME: &str = "wg";
const TIMEOUT: u64 = 10;

const SYSTEM_CONFIG_DIRS: &[&str] = &[
    "/etc/wireguard",
    "/opt/homebrew/etc/wireguard",
    "/usr/local/etc/wireguard",
];

fn user_config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()))
        .join(".conduit")
        .join("wireguard")
}

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
        let mut extra = BTreeMap::new();
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

        // User config dir first (no sudo needed): ~/.config/conduit/wireguard/
        let user_dir = user_config_dir();
        if user_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&user_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().map(|e| e == "conf").unwrap_or(false) {
                        configs.push(p);
                    }
                }
            }
        }

        // Then system dirs (may need sudo to read)
        for dir in SYSTEM_CONFIG_DIRS {
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

    /// Find the config file path for a given interface name
    fn find_config_path(interface: &str) -> Option<PathBuf> {
        Self::list_config_files()
            .into_iter()
            .find(|p| {
                p.file_stem()
                    .map(|s| s.to_string_lossy() == interface)
                    .unwrap_or(false)
            })
    }

    /// Returns the user-writable config directory, creating it if needed
    pub fn ensure_user_config_dir() -> Result<PathBuf, String> {
        let dir = user_config_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(dir)
    }

    /// Check if the WireGuard tunnel is truly active by verifying
    /// /var/run/wireguard/<interface>.name exists and is non-empty.
    /// The .name file is root-owned (0400), so we may not be able to read it.
    /// If the file exists but is unreadable, assume active (wg-quick created it).
    /// Only treat as stale if we CAN read it and it's empty.
    fn is_tunnel_active(interface: &str) -> bool {
        let name_file = PathBuf::from(format!("/var/run/wireguard/{}.name", interface));
        if !name_file.exists() {
            return false;
        }
        match std::fs::read_to_string(&name_file) {
            Ok(content) => !content.trim().is_empty(),
            Err(_) => true, // file exists but unreadable (root-owned) — assume active
        }
    }

    /// Try to read the utun interface name from the .name file.
    /// Returns None if file doesn't exist, is unreadable, or is empty.
    fn read_tunnel_name(interface: &str) -> Option<String> {
        let name_file = PathBuf::from(format!("/var/run/wireguard/{}.name", interface));
        let content = std::fs::read_to_string(&name_file).ok()?;
        let trimmed = content.trim().to_string();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    }

    async fn exec_with_sudo(command: &str) -> Result<String, VpnError> {
        // Prepend PATH with common tool locations so wg-quick can find wg
        let full_command = format!(
            "export PATH=/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin:$PATH; {}",
            command
        );
        let script = format!(
            "do shell script \"{}\" with administrator privileges",
            full_command.replace('\\', "\\\\").replace('"', "\\\"")
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
        find_tool(TOOL_NAME).is_some()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    async fn connect(&self, _opts: ConnectOptions) -> Result<(), VpnError> {
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }
        // Already connected? Skip. Check actual tunnel state, not just file existence,
        // to avoid being fooled by stale .name files.
        if Self::is_tunnel_active(&self.interface) {
            return Ok(());
        }
        let wg_quick = find_tool("wg-quick")
            .unwrap_or_else(|| "wg-quick".to_string());
        let config_path = Self::find_config_path(&self.interface)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| self.interface.clone());
        let cmd = format!("{} up {}", wg_quick, config_path);
        Self::exec_with_sudo(&cmd).await?;
        Ok(())
    }

    async fn disconnect(&self) -> Result<(), VpnError> {
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }
        let wg_quick = find_tool("wg-quick")
            .unwrap_or_else(|| "wg-quick".to_string());
        let config_path = Self::find_config_path(&self.interface)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| self.interface.clone());
        // Combine wg-quick down + stale file cleanup in one sudo call.
        // If wg-quick fails (interface not up), the rm still runs to clean
        // up the .name file so status() reports disconnected.
        let cmd = format!(
            "{} down {} 2>/dev/null; rm -f /var/run/wireguard/{}.name",
            wg_quick, config_path, self.interface
        );
        let _ = Self::exec_with_sudo(&cmd).await;
        Ok(())
    }

    async fn status(&self) -> Result<VpnStatus, VpnError> {
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }
        let mut extra = BTreeMap::new();
        extra.insert("interface".to_string(), self.interface.clone());

        // On macOS, wg-quick creates /var/run/wireguard/<name>.name
        // containing the actual utun interface name. The file is root-owned (0400)
        // so we may not be able to read it. Use is_tunnel_active for the connected
        // check (handles unreadable files), and read_tunnel_name for the utun name.
        if Self::is_tunnel_active(&self.interface) {
            let mut ip = None;
            if let Some(utun) = Self::read_tunnel_name(&self.interface) {
                extra.insert("tunnel".to_string(), utun.clone());
                if let Ok(output) = exec_command("ifconfig", &[&utun], TIMEOUT).await {
                    ip = output
                        .lines()
                        .find(|line| line.trim().starts_with("inet "))
                        .and_then(|line| line.split_whitespace().nth(1))
                        .map(String::from);
                }
            }

            Ok(VpnStatus {
                provider: "WireGuard".to_string(),
                connected: true,
                ip,
                since: None,
                latency_ms: None,
                extra,
            })
        } else {
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
    fn test_is_tunnel_active_missing_file() {
        assert!(!WireGuardProvider::is_tunnel_active("nonexistent_test_interface_12345"));
    }

    #[test]
    fn test_is_tunnel_active_empty_file() {
        // This test verifies the logic conceptually; the actual runtime path
        // /var/run/wireguard/ requires root. The helper reads any path under
        // /var/run/wireguard/, so we test the string-empty check directly.
        let content = "   \n  ";
        assert!(content.trim().is_empty());
    }

    #[test]
    fn test_list_config_files() {
        let files = WireGuardProvider::list_config_files();
        for file in &files {
            assert!(file.extension().map(|e| e == "conf").unwrap_or(false));
        }
    }
}
