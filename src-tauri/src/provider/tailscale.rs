use async_trait::async_trait;
use std::any::Any;
use std::collections::BTreeMap;

use crate::provider::{
    ConnectOptions, ProviderConfig, VpnError, VpnProvider, VpnStatus,
};
use crate::util::detect::find_tool;
use crate::util::exec::exec_command;

const TOOL_NAME: &str = "tailscale";
const MAC_APP_CLI: &str = "/Applications/Tailscale.app/Contents/MacOS/Tailscale";
const TIMEOUT: u64 = 10;

pub struct TailscaleProvider {
    cli_path: Option<String>,
}

impl TailscaleProvider {
    pub fn new() -> Self {
        // Prefer the Mac App's bundled CLI (talks to the Mac App daemon)
        // Fall back to standalone `tailscale` CLI from brew/PATH
        let cli_path = if std::path::Path::new(MAC_APP_CLI).exists() {
            Some(MAC_APP_CLI.to_string())
        } else {
            find_tool(TOOL_NAME)
        };
        Self { cli_path }
    }

    fn cli(&self) -> Result<&str, VpnError> {
        self.cli_path.as_deref().ok_or(VpnError::NotInstalled)
    }

    fn parse_status(json_str: &str) -> Result<VpnStatus, VpnError> {
        let v: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| VpnError::ParseError(e.to_string()))?;

        let backend_state = v["BackendState"].as_str().unwrap_or("Unknown");
        let connected = backend_state == "Running";

        let ips = v["Self"]["TailscaleIPs"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|ip| ip.as_str())
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let ip = ips.first().cloned();

        let mut extra = BTreeMap::new();

        if let Some(hostname) = v["Self"]["HostName"].as_str() {
            extra.insert("hostname".to_string(), hostname.to_string());
        }

        if let Some(tailnet) = v["CurrentTailnet"]["Name"].as_str() {
            extra.insert("tailnet_name".to_string(), tailnet.to_string());
        }

        if let Some(peers) = v["Peer"].as_object() {
            extra.insert("peers_count".to_string(), peers.len().to_string());
        }

        if let Some(exit_node) = v["ExitNodeStatus"]["ID"].as_str() {
            extra.insert("exit_node".to_string(), exit_node.to_string());
        }

        Ok(VpnStatus {
            provider: "Tailscale".to_string(),
            connected,
            ip,
            since: None,
            latency_ms: None,
            extra,
        })
    }
}

#[async_trait]
impl VpnProvider for TailscaleProvider {
    fn name(&self) -> &str {
        "Tailscale"
    }

    fn is_installed(&self) -> bool {
        self.cli_path.is_some()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    async fn connect(&self, opts: ConnectOptions) -> Result<(), VpnError> {
        let cli = self.cli()?;
        let mut args = vec!["up"];

        let exit_node_arg;
        if let Some(ProviderConfig::Tailscale { exit_node: Some(ref node), .. }) = opts.provider_config {
            exit_node_arg = format!("--exit-node={}", node);
            args.push(&exit_node_arg);
        }

        exec_command(cli, &args, TIMEOUT).await?;
        Ok(())
    }

    async fn disconnect(&self) -> Result<(), VpnError> {
        let cli = self.cli()?;
        exec_command(cli, &["down"], TIMEOUT).await?;
        Ok(())
    }

    async fn status(&self) -> Result<VpnStatus, VpnError> {
        let cli = self.cli()?;
        let output = exec_command(cli, &["status", "--json"], TIMEOUT).await?;
        Self::parse_status(&output)
    }

    async fn get_config(&self) -> Result<ProviderConfig, VpnError> {
        Ok(ProviderConfig::Tailscale {
            exit_node: None,
            accept_routes: false,
            shields_up: false,
        })
    }

    async fn set_config(&self, config: ProviderConfig) -> Result<(), VpnError> {
        let cli = self.cli()?;
        if let ProviderConfig::Tailscale { accept_routes, shields_up, .. } = config {
            let mut args = vec!["set"];
            let accept_routes_flag = format!("--accept-routes={}", accept_routes);
            args.push(&accept_routes_flag);
            let shields_up_flag = format!("--shields-up={}", shields_up);
            args.push(&shields_up_flag);
            exec_command(cli, &args, TIMEOUT).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_connected() {
        let json = r#"{
            "Version": "1.60.0",
            "BackendState": "Running",
            "Self": {
                "ID": "abc123",
                "HostName": "my-mac",
                "DNSName": "my-mac.tailnet.ts.net.",
                "TailscaleIPs": ["100.64.0.1", "fd7a:115c:a1e0::1"]
            },
            "CurrentTailnet": {
                "Name": "myuser@github",
                "MagicDNSSuffix": "tailnet.ts.net"
            },
            "Peer": {
                "peer1": {"ID": "p1", "HostName": "peer1"},
                "peer2": {"ID": "p2", "HostName": "peer2"}
            },
            "ExitNodeStatus": null
        }"#;

        let status = TailscaleProvider::parse_status(json).unwrap();
        assert!(status.connected);
        assert_eq!(status.provider, "Tailscale");
        assert_eq!(status.ip.as_deref(), Some("100.64.0.1"));
        assert_eq!(status.extra.get("tailnet_name").map(|s| s.as_str()), Some("myuser@github"));
        assert_eq!(status.extra.get("hostname").map(|s| s.as_str()), Some("my-mac"));
        assert_eq!(status.extra.get("peers_count").map(|s| s.as_str()), Some("2"));
    }

    #[test]
    fn test_parse_status_disconnected() {
        let json = r#"{
            "Version": "1.60.0",
            "BackendState": "Stopped",
            "Self": {
                "ID": "abc123",
                "HostName": "my-mac",
                "DNSName": "my-mac.tailnet.ts.net.",
                "TailscaleIPs": []
            },
            "CurrentTailnet": null,
            "Peer": {}
        }"#;

        let status = TailscaleProvider::parse_status(json).unwrap();
        assert!(!status.connected);
        assert!(status.ip.is_none());
    }

    #[test]
    fn test_parse_status_invalid_json() {
        let result = TailscaleProvider::parse_status("not json");
        assert!(matches!(result, Err(VpnError::ParseError(_))));
    }
}
