use async_trait::async_trait;
use std::any::Any;
use std::collections::BTreeMap;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::provider::{ConnectOptions, ProviderConfig, VpnError, VpnProvider, VpnStatus};

const SOCKET_PATH: &str = "/var/run/pritunl.sock";
const AUTH_PATH: &str = "/var/run/pritunl.auth";
const APP_CLI: &str = "/Applications/Pritunl.app/Contents/Resources/pritunl-client";

fn profiles_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join("Library")
                .join("Application Support")
        })
        .join("pritunl")
        .join("profiles")
}

fn read_auth_key() -> Result<String, VpnError> {
    std::fs::read_to_string(AUTH_PATH)
        .map(|s| s.trim().to_string())
        .map_err(|e| VpnError::CliError(format!("Failed to read auth key: {}", e)))
}

/// Make an HTTP request to the Pritunl service over its Unix socket.
async fn service_request(
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<(u16, String), VpnError> {
    let auth_key = read_auth_key()?;

    let mut stream = UnixStream::connect(SOCKET_PATH)
        .await
        .map_err(|e| VpnError::CliError(format!("Failed to connect to pritunl service: {}", e)))?;

    let body_bytes = body.unwrap_or("");
    let request = format!(
        "{} {} HTTP/1.1\r\n\
         Host: unix\r\n\
         Auth-Key: {}\r\n\
         User-Agent: pritunl\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        method,
        path,
        auth_key,
        body_bytes.len(),
        body_bytes
    );

    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| VpnError::CliError(format!("Failed to write to service: {}", e)))?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .map_err(|e| VpnError::CliError(format!("Failed to read from service: {}", e)))?;

    let response_str = String::from_utf8_lossy(&response);

    // Parse HTTP response: status line then headers then body
    let mut parts = response_str.splitn(2, "\r\n\r\n");
    let header_section = parts.next().unwrap_or("");
    let body_section = parts.next().unwrap_or("");

    let status_code = header_section
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0);

    Ok((status_code, body_section.to_string()))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PritunlProfile {
    pub id: String,
    pub name: String,
    pub server: String,
    pub organization: String,
    pub user: String,
    pub password_mode: Option<String>,
}

/// Read all Pritunl profiles from the GUI's profile directory.
pub fn list_profiles() -> Vec<PritunlProfile> {
    let dir = profiles_dir();
    let mut profiles = Vec::new();

    if !dir.is_dir() {
        return profiles;
    }

    let Ok(entries) = std::fs::read_dir(&dir) else {
        return profiles;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "conf").unwrap_or(false) {
            let id = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                    let server = v["server"].as_str().unwrap_or("").to_string();
                    let org = v["organization"].as_str().unwrap_or("").to_string();
                    let user = v["user"].as_str().unwrap_or("").to_string();
                    let password_mode = v["password_mode"].as_str().map(String::from);

                    // Skip profiles with no server name
                    let name = if !server.is_empty() {
                        server.clone()
                    } else {
                        id.clone()
                    };

                    profiles.push(PritunlProfile {
                        id,
                        name,
                        server,
                        organization: org,
                        user,
                        password_mode,
                    });
                }
            }
        }
    }

    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    profiles
}

/// Build the JSON body for a connect request from a profile's .conf file.
/// Read the full .ovpn data, including any private key stored in macOS keychain.
fn read_ovpn_data(profile_id: &str) -> Result<String, VpnError> {
    let ovpn_path = profiles_dir().join(format!("{}.ovpn", profile_id));
    let mut data = std::fs::read_to_string(&ovpn_path)
        .map_err(|e| VpnError::CliError(format!("Failed to read ovpn file: {}", e)))?;

    // Try to read additional key data from macOS keychain (some profiles store keys there)
    if let Ok(output) = std::process::Command::new("/usr/bin/security")
        .args(["find-generic-password", "-w", "-s", "pritunl", "-a", profile_id])
        .output()
    {
        if output.status.success() {
            let encoded = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !encoded.is_empty() {
                if let Ok(decoded) = base64_decode(&encoded) {
                    data.push_str(&decoded);
                }
            }
        }
    }

    Ok(data)
}

fn base64_decode(input: &str) -> Result<String, VpnError> {
    use std::io::Read;
    // Simple base64 decode using the standard library approach
    let mut output = Vec::new();
    let clean: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let lookup = |c: u8| -> Result<u8, VpnError> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0),
            _ => Err(VpnError::ParseError("Invalid base64".to_string())),
        }
    };
    let bytes = clean.as_bytes();
    for chunk in bytes.chunks(4) {
        if chunk.len() < 2 { break; }
        let a = lookup(chunk[0])?;
        let b = lookup(chunk[1])?;
        output.push((a << 2) | (b >> 4));
        if chunk.len() > 2 && chunk[2] != b'=' {
            let c = lookup(chunk[2])?;
            output.push((b << 4) | (c >> 2));
            if chunk.len() > 3 && chunk[3] != b'=' {
                let d = lookup(chunk[3])?;
                output.push((c << 6) | d);
            }
        }
    }
    String::from_utf8(output).map_err(|e| VpnError::ParseError(e.to_string()))
}

fn build_connect_body(
    profile_id: &str,
    password: Option<&str>,
) -> Result<String, VpnError> {
    let conf_path = profiles_dir().join(format!("{}.conf", profile_id));
    let content = std::fs::read_to_string(&conf_path)
        .map_err(|e| VpnError::CliError(format!("Failed to read profile conf: {}", e)))?;
    let v: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| VpnError::ParseError(format!("Failed to parse profile conf: {}", e)))?;

    let mode = if v["wg"].as_bool().unwrap_or(false) {
        "wg"
    } else {
        "ovpn"
    };

    // Read the full .ovpn data (same as the Electron GUI does)
    let ovpn_data = read_ovpn_data(profile_id)?;

    // server_public_key is stored as an array of lines, join with newlines
    let server_pub_key: serde_json::Value = match v["server_public_key"].as_array() {
        Some(arr) => {
            let joined: String = arr
                .iter()
                .filter_map(|l| l.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            serde_json::Value::String(joined)
        }
        None => serde_json::Value::Null,
    };

    let body = serde_json::json!({
        "id": profile_id,
        "mode": mode,
        "org_id": v["organization_id"].as_str().unwrap_or(""),
        "user_id": v["user_id"].as_str().unwrap_or(""),
        "server_id": v["server_id"].as_str().unwrap_or(""),
        "sync_hosts": v["sync_hosts"],
        "sync_token": v["sync_token"].as_str().unwrap_or(""),
        "sync_secret": v["sync_secret"].as_str().unwrap_or(""),
        "username": "pritunl",
        "password": password.unwrap_or(""),
        "dynamic_firewall": v["dynamic_firewall"].as_bool().unwrap_or(false),
        "device_auth": v["device_auth"].as_bool().unwrap_or(false),
        "sso_auth": v["sso_auth"].as_bool().unwrap_or(false),
        "server_public_key": server_pub_key,
        "server_box_public_key": v["server_box_public_key"],
        "token_ttl": v["token_ttl"].as_i64().unwrap_or(2592000),
        "reconnect": !v["disable_reconnect"].as_bool().unwrap_or(false),
        "timeout": false,
        "data": ovpn_data,
    });

    serde_json::to_string(&body)
        .map_err(|e| VpnError::ParseError(format!("Failed to serialize connect body: {}", e)))
}

pub struct PritunlProvider {
    pub profile_id: String,
}

impl PritunlProvider {
    pub fn new() -> Self {
        Self {
            profile_id: String::new(),
        }
    }

    pub fn with_profile(profile_id: &str) -> Self {
        Self {
            profile_id: profile_id.to_string(),
        }
    }
}

#[async_trait]
impl VpnProvider for PritunlProvider {
    fn name(&self) -> &str {
        "Pritunl"
    }

    fn is_installed(&self) -> bool {
        std::path::Path::new(APP_CLI).exists()
            && std::path::Path::new(SOCKET_PATH).exists()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    async fn connect(&self, opts: ConnectOptions) -> Result<(), VpnError> {
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }
        if self.profile_id.is_empty() {
            return Err(VpnError::CliError("No Pritunl profile selected".to_string()));
        }

        // Extract password from connect options if provided
        let password = opts
            .provider_config
            .as_ref()
            .and_then(|c| {
                if let ProviderConfig::Pritunl { ref password, .. } = c {
                    password.clone()
                } else {
                    None
                }
            });

        let body = build_connect_body(
            &self.profile_id,
            password.as_deref(),
        )?;

        let (status, resp_body) = service_request("POST", "/profile", Some(&body)).await?;
        if status != 200 {
            return Err(VpnError::CliError(format!(
                "Pritunl connect failed ({}): {}",
                status, resp_body
            )));
        }
        Ok(())
    }

    async fn disconnect(&self) -> Result<(), VpnError> {
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }
        if self.profile_id.is_empty() {
            return Ok(());
        }

        let body = serde_json::json!({ "id": self.profile_id }).to_string();
        let (status, resp_body) = service_request("DELETE", "/profile", Some(&body)).await?;
        if status != 200 {
            return Err(VpnError::CliError(format!(
                "Pritunl disconnect failed ({}): {}",
                status, resp_body
            )));
        }
        Ok(())
    }

    async fn status(&self) -> Result<VpnStatus, VpnError> {
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }

        let mut extra = BTreeMap::new();
        if !self.profile_id.is_empty() {
            extra.insert("profile".to_string(), self.profile_id.clone());

            // Add profile metadata
            let profiles = list_profiles();
            if let Some(p) = profiles.iter().find(|p| p.id == self.profile_id) {
                extra.insert("server".to_string(), p.name.clone());
                if !p.user.is_empty() {
                    extra.insert("user".to_string(), p.user.clone());
                }
                if let Some(ref mode) = p.password_mode {
                    extra.insert("password_mode".to_string(), mode.clone());
                }
            }
        }

        // Query the service for active connections
        let (status_code, body) = match service_request("GET", "/profile", None).await {
            Ok(r) => r,
            Err(_) => {
                return Ok(VpnStatus {
                    provider: "Pritunl".to_string(),
                    connected: false,
                    ip: None,
                    since: None,
                    latency_ms: None,
                    extra,
                });
            }
        };

        if status_code != 200 {
            return Ok(VpnStatus {
                provider: "Pritunl".to_string(),
                connected: false,
                ip: None,
                since: None,
                latency_ms: None,
                extra,
            });
        }

        // Parse the response — it's a JSON object with profile IDs as keys
        let v: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();

        let mut connected = false;
        let mut ip = None;

        // Check if our profile or any profile is connected
        if let Some(obj) = v.as_object() {
            for (pid, profile_data) in obj {
                let profile_status = profile_data["status"].as_str().unwrap_or("");
                let is_connected = profile_status == "connected";

                if pid == &self.profile_id || self.profile_id.is_empty() {
                    if is_connected {
                        connected = true;
                        if let Some(addr) = profile_data["client_address"].as_str() {
                            if !addr.is_empty() {
                                ip = Some(addr.to_string());
                            }
                        }
                        if let Some(server_addr) = profile_data["server_address"].as_str() {
                            if !server_addr.is_empty() {
                                extra.insert(
                                    "server_address".to_string(),
                                    server_addr.to_string(),
                                );
                            }
                        }
                        break;
                    }
                }
            }
        }

        Ok(VpnStatus {
            provider: "Pritunl".to_string(),
            connected,
            ip,
            since: None,
            latency_ms: None,
            extra,
        })
    }

    async fn get_config(&self) -> Result<ProviderConfig, VpnError> {
        Ok(ProviderConfig::Pritunl {
            profile_id: self.profile_id.clone(),
            password: None,
        })
    }

    async fn set_config(&self, _config: ProviderConfig) -> Result<(), VpnError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_profiles() {
        // Just verify it doesn't panic — actual results depend on system state
        let profiles = list_profiles();
        for p in &profiles {
            assert!(!p.id.is_empty());
        }
    }

    #[test]
    fn test_profiles_dir() {
        let dir = profiles_dir();
        assert!(dir.to_string_lossy().contains("pritunl"));
    }
}
