use async_trait::async_trait;
use std::any::Any;
use std::collections::BTreeMap;

use crate::provider::{
    ConnectOptions, ProviderConfig, VpnError, VpnProvider, VpnStatus, WarpMode,
};
use crate::util::detect::find_tool;
use crate::util::exec::exec_command;

const TOOL_NAME: &str = "warp-cli";
const TIMEOUT: u64 = 10;

pub struct WarpProvider {
    cli_path: Option<String>,
}

impl WarpProvider {
    pub fn new() -> Self {
        Self {
            cli_path: find_tool(TOOL_NAME),
        }
    }

    fn cli(&self) -> Result<&str, VpnError> {
        self.cli_path.as_deref().ok_or(VpnError::NotInstalled)
    }

    fn parse_status(output: &str) -> Result<VpnStatus, VpnError> {
        let mut extra = BTreeMap::new();

        let status_line = output
            .lines()
            .find(|line| line.starts_with("Status update:"))
            .ok_or_else(|| VpnError::ParseError("No status line found".to_string()))?;

        let status_value = status_line
            .trim_start_matches("Status update:")
            .trim();

        let connected = status_value == "Connected";

        for line in output.lines() {
            if let Some(mode) = line.strip_prefix("Mode:") {
                extra.insert("warp_mode".to_string(), mode.trim().to_string());
            }
            if let Some(dns) = line.strip_prefix("DnsProxy:") {
                extra.insert("dns_proxy".to_string(), dns.trim().to_string());
            }
        }

        Ok(VpnStatus {
            provider: "WARP".to_string(),
            connected,
            ip: None,
            since: None,
            latency_ms: None,
            extra,
        })
    }
}

#[async_trait]
impl VpnProvider for WarpProvider {
    fn name(&self) -> &str {
        "WARP"
    }

    fn is_installed(&self) -> bool {
        self.cli_path.is_some()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    async fn connect(&self, _opts: ConnectOptions) -> Result<(), VpnError> {
        let cli = self.cli()?;
        exec_command(cli, &["connect"], TIMEOUT).await?;
        Ok(())
    }

    async fn disconnect(&self) -> Result<(), VpnError> {
        let cli = self.cli()?;
        exec_command(cli, &["disconnect"], TIMEOUT).await?;
        Ok(())
    }

    async fn status(&self) -> Result<VpnStatus, VpnError> {
        let cli = self.cli()?;
        let output = exec_command(cli, &["status"], TIMEOUT).await
            .or_else(|e| {
                if let crate::util::exec::ExecError::NonZeroExit { stderr, .. } = &e {
                    if stderr.is_empty() {
                        return Err(VpnError::from(e));
                    }
                }
                Err(VpnError::from(e))
            })?;
        Self::parse_status(&output)
    }

    async fn get_config(&self) -> Result<ProviderConfig, VpnError> {
        Ok(ProviderConfig::Warp {
            mode: WarpMode::Warp,
            families_mode: false,
        })
    }

    async fn set_config(&self, config: ProviderConfig) -> Result<(), VpnError> {
        let cli = self.cli()?;
        if let ProviderConfig::Warp { mode, families_mode } = config {
            let mode_arg = match mode {
                WarpMode::Warp => "warp",
                WarpMode::DnsOnly => "doh",
                WarpMode::Proxy => "proxy",
            };
            exec_command(cli, &["mode", mode_arg], TIMEOUT).await?;

            let families_arg = if families_mode { "malware" } else { "off" };
            exec_command(cli, &["dns", "families", families_arg], TIMEOUT).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_connected() {
        let output = "Status update: Connected\n\
                       DnsProxy: false\n\
                       Mode: Warp\n\
                       Reason: Manual Connection";

        let status = WarpProvider::parse_status(output).unwrap();
        assert!(status.connected);
        assert_eq!(status.provider, "WARP");
        assert_eq!(status.extra.get("warp_mode").map(|s| s.as_str()), Some("Warp"));
    }

    #[test]
    fn test_parse_status_disconnected() {
        let output = "Status update: Disconnected\n\
                       Reason: Manual Disconnection";

        let status = WarpProvider::parse_status(output).unwrap();
        assert!(!status.connected);
        assert_eq!(status.provider, "WARP");
    }

    #[test]
    fn test_parse_status_connecting() {
        let output = "Status update: Connecting\n\
                       Reason: Registration";

        let status = WarpProvider::parse_status(output).unwrap();
        assert!(!status.connected);
    }

    #[test]
    fn test_parse_status_empty() {
        let result = WarpProvider::parse_status("");
        assert!(matches!(result, Err(VpnError::ParseError(_))));
    }
}
