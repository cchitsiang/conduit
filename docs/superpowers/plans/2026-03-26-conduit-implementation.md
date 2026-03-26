# Conduit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a macOS menu bar + dashboard app that manages connect/disconnect status, displays connection details, and configures Tailscale, Cloudflare WARP, and WireGuard through their CLIs.

**Architecture:** Single Tauri 2 process with a Rust backend implementing a `VpnProvider` trait per VPN tool (CLI wrappers). A Tokio polling loop pushes status changes to a SvelteKit frontend via Tauri events. Menu bar tray for quick toggles, dashboard window for full management.

**Tech Stack:** Tauri 2.x, Rust (Tokio, Serde, async-trait, chrono), SvelteKit 5, TypeScript, Tailwind CSS 4, Vite

**Spec:** `docs/superpowers/specs/2026-03-26-conduit-design.md`

---

## File Map

### Rust Backend (`src-tauri/src/`)

| File | Responsibility |
|------|---------------|
| `main.rs` | Tauri app entry point — registers commands, sets up tray, initializes state, spawns poll loop |
| `commands.rs` | All `#[tauri::command]` IPC handlers — delegates to `AppState` |
| `state.rs` | `AppState` struct holding providers + settings, polling loop, mutex-guarded operations |
| `tray.rs` | System tray construction, menu building, tray event handling |
| `settings.rs` | `AppSettings` struct, load/save to JSON file |
| `provider/mod.rs` | `VpnProvider` trait, `VpnStatus`, `ProviderConfig`, `VpnError`, `ConnectOptions` types |
| `provider/tailscale.rs` | `TailscaleProvider` — implements `VpnProvider` via `tailscale` CLI |
| `provider/warp.rs` | `WarpProvider` — implements `VpnProvider` via `warp-cli` |
| `provider/wireguard.rs` | `WireGuardProvider` — implements `VpnProvider` via `wg`/`wg-quick` CLI |
| `util/mod.rs` | Re-exports for `exec` and `detect` |
| `util/exec.rs` | `exec_command()` — async CLI runner with timeout, stdout/stderr capture |
| `util/detect.rs` | `is_tool_installed()` — checks if a CLI tool exists on PATH |

### Frontend (`src/`)

| File | Responsibility |
|------|---------------|
| `app.html` | HTML shell (Tauri default) |
| `app.css` | Tailwind CSS 4 import + global styles |
| `routes/+layout.ts` | Disables SSR (`export const ssr = false`) |
| `routes/+layout.svelte` | Root layout — imports app.css |
| `routes/+page.svelte` | Dashboard page — renders VPN cards + activity log |
| `lib/tauri.ts` | Typed wrappers around `invoke()` and `listen()` |
| `lib/types.ts` | TypeScript types mirroring Rust types (`VpnStatus`, `ProviderConfig`, etc.) |
| `lib/stores/vpn.ts` | Svelte stores for VPN statuses, activity log, settings |
| `lib/components/StatusDot.svelte` | Colored status indicator (green/grey/yellow/red) |
| `lib/components/Toggle.svelte` | Connect/disconnect toggle switch |
| `lib/components/ConfigPanel.svelte` | Expandable config section per provider |
| `lib/components/VpnCard.svelte` | Full provider card — status, toggle, config, details |
| `lib/components/ActivityLog.svelte` | Rolling event log display |
| `lib/components/SettingsPanel.svelte` | Settings modal (poll interval, visibility, launch at login) |

---

## Task 1: Project Scaffolding

**Files:**
- Create: `src-tauri/`, `src/`, `package.json`, `svelte.config.js`, `vite.config.ts`, `tsconfig.json`

- [ ] **Step 1: Scaffold Tauri 2 + SvelteKit project**

Run from the `conduit/` directory. Move existing `docs/` out first, scaffold, then move `docs/` back:

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
mv docs /tmp/conduit-docs-backup
# Remove .git so scaffold can init fresh
rm -rf .git
cd ..
npm create tauri-app@latest conduit -- --template sveltekit-ts --manager npm
cd conduit
mv /tmp/conduit-docs-backup ./docs
git init
```

If the interactive scaffold doesn't support `--template sveltekit-ts`, run it interactively and select: SvelteKit, TypeScript, npm.

- [ ] **Step 2: Install Tailwind CSS 4**

```bash
npm install -D tailwindcss @tailwindcss/vite
```

- [ ] **Step 3: Configure Tailwind Vite plugin**

Update `vite.config.ts`:

```typescript
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
});
```

- [ ] **Step 4: Set up app.css with Tailwind**

Replace contents of `src/app.css`:

```css
@import "tailwindcss";
```

- [ ] **Step 5: Create root layout files**

Create `src/routes/+layout.ts`:

```typescript
export const ssr = false;
```

Create `src/routes/+layout.svelte`:

```svelte
<script>
  import "../app.css";
  let { children } = $props();
</script>

{@render children()}
```

- [ ] **Step 6: Update Cargo.toml with required dependencies**

Edit `src-tauri/Cargo.toml` dependencies section:

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-opener = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["process", "time", "sync"] }
async-trait = "0.1"
chrono = { version = "0.4", features = ["serde"] }

[build-dependencies]
tauri-build = { version = "2", features = [] }
```

- [ ] **Step 7: Verify the project builds**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
npm install
cd src-tauri && cargo check && cd ..
npm run build
```

Expected: All three commands succeed without errors.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: scaffold Tauri 2 + SvelteKit + Tailwind CSS 4 project"
```

---

## Task 2: CLI Execution Utility

**Files:**
- Create: `src-tauri/src/util/mod.rs`
- Create: `src-tauri/src/util/exec.rs`
- Create: `src-tauri/src/util/detect.rs`

- [ ] **Step 1: Write tests for exec_command**

Create `src-tauri/src/util/exec.rs`:

```rust
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug)]
pub enum ExecError {
    Timeout,
    IoError(String),
    NonZeroExit { code: Option<i32>, stderr: String },
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecError::Timeout => write!(f, "Command timed out"),
            ExecError::IoError(e) => write!(f, "IO error: {}", e),
            ExecError::NonZeroExit { code, stderr } => {
                write!(f, "Exit code {:?}: {}", code, stderr)
            }
        }
    }
}

pub async fn exec_command(
    program: &str,
    args: &[&str],
    timeout_secs: u64,
) -> Result<String, ExecError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_exec_command_success() {
        let result = exec_command("echo", &["hello"], 10).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().trim(), "hello");
    }

    #[tokio::test]
    async fn test_exec_command_nonzero_exit() {
        let result = exec_command("false", &[], 10).await;
        assert!(matches!(result, Err(ExecError::NonZeroExit { .. })));
    }

    #[tokio::test]
    async fn test_exec_command_timeout() {
        let result = exec_command("sleep", &["30"], 1).await;
        assert!(matches!(result, Err(ExecError::Timeout)));
    }

    #[tokio::test]
    async fn test_exec_command_not_found() {
        let result = exec_command("nonexistent_binary_xyz", &[], 10).await;
        assert!(matches!(result, Err(ExecError::IoError(_))));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo test util::exec::tests -- --nocapture
```

Expected: All 4 tests fail with `not yet implemented`.

- [ ] **Step 3: Implement exec_command**

Replace the `todo!()` in `exec_command`:

```rust
pub async fn exec_command(
    program: &str,
    args: &[&str],
    timeout_secs: u64,
) -> Result<String, ExecError> {
    let duration = Duration::from_secs(timeout_secs);

    let future = Command::new(program)
        .args(args)
        .output();

    let output = timeout(duration, future)
        .await
        .map_err(|_| ExecError::Timeout)?
        .map_err(|e| ExecError::IoError(e.to_string()))?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .map_err(|e| ExecError::IoError(e.to_string()))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(ExecError::NonZeroExit {
            code: output.status.code(),
            stderr,
        })
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo test util::exec::tests -- --nocapture
```

Expected: All 4 tests PASS.

- [ ] **Step 5: Write tests for detect and implement**

Create `src-tauri/src/util/detect.rs`:

```rust
use std::process::Command;

pub fn is_tool_installed(tool_name: &str) -> bool {
    Command::new("which")
        .arg(tool_name)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn get_tool_path(tool_name: &str) -> Option<String> {
    Command::new("which")
        .arg(tool_name)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_existing_tool() {
        assert!(is_tool_installed("echo"));
    }

    #[test]
    fn test_detect_missing_tool() {
        assert!(!is_tool_installed("nonexistent_tool_xyz_123"));
    }

    #[test]
    fn test_get_tool_path_existing() {
        let path = get_tool_path("echo");
        assert!(path.is_some());
        assert!(path.unwrap().contains("echo"));
    }

    #[test]
    fn test_get_tool_path_missing() {
        assert!(get_tool_path("nonexistent_tool_xyz_123").is_none());
    }
}
```

- [ ] **Step 6: Create util/mod.rs**

Create `src-tauri/src/util/mod.rs`:

```rust
pub mod detect;
pub mod exec;
```

- [ ] **Step 7: Register util module in main.rs**

Add to top of `src-tauri/src/main.rs`:

```rust
mod util;
```

- [ ] **Step 8: Run all tests**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo test -- --nocapture
```

Expected: All 8 tests PASS.

- [ ] **Step 9: Commit**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
git add src-tauri/src/util/
git commit -m "feat: add CLI execution utility with timeout and tool detection"
```

---

## Task 3: VpnProvider Trait & Shared Types

**Files:**
- Create: `src-tauri/src/provider/mod.rs`

- [ ] **Step 1: Define the VpnProvider trait and all shared types**

Create `src-tauri/src/provider/mod.rs`:

```rust
pub mod tailscale;
pub mod warp;
pub mod wireguard;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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
    async fn connect(&self, opts: ConnectOptions) -> Result<(), VpnError>;
    async fn disconnect(&self) -> Result<(), VpnError>;
    async fn status(&self) -> Result<VpnStatus, VpnError>;
    async fn get_config(&self) -> Result<ProviderConfig, VpnError>;
    async fn set_config(&self, config: ProviderConfig) -> Result<(), VpnError>;
}
```

- [ ] **Step 2: Register provider module in main.rs**

Add to `src-tauri/src/main.rs`:

```rust
mod provider;
```

- [ ] **Step 3: Create placeholder provider files**

Create `src-tauri/src/provider/tailscale.rs`:

```rust
// Implemented in Task 4
```

Create `src-tauri/src/provider/warp.rs`:

```rust
// Implemented in Task 5
```

Create `src-tauri/src/provider/wireguard.rs`:

```rust
// Implemented in Task 6
```

- [ ] **Step 4: Verify it compiles**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo check
```

Expected: Compiles successfully.

- [ ] **Step 5: Commit**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
git add src-tauri/src/provider/
git commit -m "feat: define VpnProvider trait and shared types"
```

---

## Task 4: Tailscale Provider

**Files:**
- Modify: `src-tauri/src/provider/tailscale.rs`

- [ ] **Step 1: Write tests for TailscaleProvider**

Replace `src-tauri/src/provider/tailscale.rs` with:

```rust
use async_trait::async_trait;
use std::collections::HashMap;

use crate::provider::{
    ConnectOptions, ProviderConfig, VpnError, VpnProvider, VpnStatus,
};
use crate::util::detect::is_tool_installed;
use crate::util::exec::exec_command;

const TOOL_NAME: &str = "tailscale";
const TIMEOUT: u64 = 10;

pub struct TailscaleProvider;

impl TailscaleProvider {
    pub fn new() -> Self {
        Self
    }

    fn parse_status(json_str: &str) -> Result<VpnStatus, VpnError> {
        todo!()
    }
}

#[async_trait]
impl VpnProvider for TailscaleProvider {
    fn name(&self) -> &str {
        "Tailscale"
    }

    fn is_installed(&self) -> bool {
        is_tool_installed(TOOL_NAME)
    }

    async fn connect(&self, opts: ConnectOptions) -> Result<(), VpnError> {
        todo!()
    }

    async fn disconnect(&self) -> Result<(), VpnError> {
        todo!()
    }

    async fn status(&self) -> Result<VpnStatus, VpnError> {
        todo!()
    }

    async fn get_config(&self) -> Result<ProviderConfig, VpnError> {
        todo!()
    }

    async fn set_config(&self, config: ProviderConfig) -> Result<(), VpnError> {
        todo!()
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
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo test provider::tailscale::tests -- --nocapture
```

Expected: FAIL with `not yet implemented`.

- [ ] **Step 3: Implement parse_status**

Replace the `parse_status` method:

```rust
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

    let mut extra = HashMap::new();

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
```

- [ ] **Step 4: Run tests to verify parse_status passes**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo test provider::tailscale::tests -- --nocapture
```

Expected: All 3 tests PASS.

- [ ] **Step 5: Implement the VpnProvider trait methods**

Replace the `todo!()` implementations in the `impl VpnProvider` block:

```rust
#[async_trait]
impl VpnProvider for TailscaleProvider {
    fn name(&self) -> &str {
        "Tailscale"
    }

    fn is_installed(&self) -> bool {
        is_tool_installed(TOOL_NAME)
    }

    async fn connect(&self, opts: ConnectOptions) -> Result<(), VpnError> {
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }

        let mut args = vec!["up"];

        let exit_node_arg;
        if let Some(ProviderConfig::Tailscale { exit_node: Some(ref node), .. }) = opts.provider_config {
            exit_node_arg = format!("--exit-node={}", node);
            args.push(&exit_node_arg);
        }

        exec_command(TOOL_NAME, &args, TIMEOUT).await?;
        Ok(())
    }

    async fn disconnect(&self) -> Result<(), VpnError> {
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }
        exec_command(TOOL_NAME, &["down"], TIMEOUT).await?;
        Ok(())
    }

    async fn status(&self) -> Result<VpnStatus, VpnError> {
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }
        let output = exec_command(TOOL_NAME, &["status", "--json"], TIMEOUT).await?;
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
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }
        if let ProviderConfig::Tailscale { accept_routes, shields_up, .. } = config {
            let mut args = vec!["set"];
            let accept_routes_flag = format!("--accept-routes={}", accept_routes);
            args.push(&accept_routes_flag);
            let shields_up_flag = format!("--shields-up={}", shields_up);
            args.push(&shields_up_flag);
            exec_command(TOOL_NAME, &args, TIMEOUT).await?;
        }
        Ok(())
    }
}
```

- [ ] **Step 6: Verify it compiles**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo check
```

Expected: Compiles successfully.

- [ ] **Step 7: Commit**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
git add src-tauri/src/provider/tailscale.rs
git commit -m "feat: implement Tailscale VPN provider with CLI wrapper"
```

---

## Task 5: WARP Provider

**Files:**
- Modify: `src-tauri/src/provider/warp.rs`

- [ ] **Step 1: Write tests for WarpProvider**

Replace `src-tauri/src/provider/warp.rs` with:

```rust
use async_trait::async_trait;
use std::collections::HashMap;

use crate::provider::{
    ConnectOptions, ProviderConfig, VpnError, VpnProvider, VpnStatus, WarpMode,
};
use crate::util::detect::is_tool_installed;
use crate::util::exec::exec_command;

const TOOL_NAME: &str = "warp-cli";
const TIMEOUT: u64 = 10;

pub struct WarpProvider;

impl WarpProvider {
    pub fn new() -> Self {
        Self
    }

    fn parse_status(output: &str) -> Result<VpnStatus, VpnError> {
        todo!()
    }
}

#[async_trait]
impl VpnProvider for WarpProvider {
    fn name(&self) -> &str {
        "WARP"
    }

    fn is_installed(&self) -> bool {
        is_tool_installed(TOOL_NAME)
    }

    async fn connect(&self, _opts: ConnectOptions) -> Result<(), VpnError> {
        todo!()
    }

    async fn disconnect(&self) -> Result<(), VpnError> {
        todo!()
    }

    async fn status(&self) -> Result<VpnStatus, VpnError> {
        todo!()
    }

    async fn get_config(&self) -> Result<ProviderConfig, VpnError> {
        todo!()
    }

    async fn set_config(&self, config: ProviderConfig) -> Result<(), VpnError> {
        todo!()
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
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo test provider::warp::tests -- --nocapture
```

Expected: FAIL with `not yet implemented`.

- [ ] **Step 3: Implement parse_status**

Replace the `parse_status` method:

```rust
fn parse_status(output: &str) -> Result<VpnStatus, VpnError> {
    let mut connected = false;
    let mut extra = HashMap::new();

    let status_line = output
        .lines()
        .find(|line| line.starts_with("Status update:"))
        .ok_or_else(|| VpnError::ParseError("No status line found".to_string()))?;

    let status_value = status_line
        .trim_start_matches("Status update:")
        .trim();

    connected = status_value == "Connected";

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
```

- [ ] **Step 4: Run tests to verify parse_status passes**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo test provider::warp::tests -- --nocapture
```

Expected: All 4 tests PASS.

- [ ] **Step 5: Implement the VpnProvider trait methods**

Replace the `todo!()` implementations:

```rust
#[async_trait]
impl VpnProvider for WarpProvider {
    fn name(&self) -> &str {
        "WARP"
    }

    fn is_installed(&self) -> bool {
        is_tool_installed(TOOL_NAME)
    }

    async fn connect(&self, _opts: ConnectOptions) -> Result<(), VpnError> {
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }
        exec_command(TOOL_NAME, &["connect"], TIMEOUT).await?;
        Ok(())
    }

    async fn disconnect(&self) -> Result<(), VpnError> {
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }
        exec_command(TOOL_NAME, &["disconnect"], TIMEOUT).await?;
        Ok(())
    }

    async fn status(&self) -> Result<VpnStatus, VpnError> {
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }
        let output = exec_command(TOOL_NAME, &["status"], TIMEOUT).await
            .or_else(|e| {
                // warp-cli status returns non-zero when disconnected but still outputs status
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
        if !self.is_installed() {
            return Err(VpnError::NotInstalled);
        }
        if let ProviderConfig::Warp { mode, families_mode } = config {
            let mode_arg = match mode {
                WarpMode::Warp => "warp",
                WarpMode::DnsOnly => "doh",
                WarpMode::Proxy => "proxy",
            };
            exec_command(TOOL_NAME, &["mode", mode_arg], TIMEOUT).await?;

            let families_arg = if families_mode { "malware" } else { "off" };
            exec_command(TOOL_NAME, &["dns", "families", families_arg], TIMEOUT).await?;
        }
        Ok(())
    }
}
```

- [ ] **Step 6: Verify it compiles**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo check
```

Expected: Compiles successfully.

- [ ] **Step 7: Commit**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
git add src-tauri/src/provider/warp.rs
git commit -m "feat: implement WARP VPN provider with CLI wrapper"
```

---

## Task 6: WireGuard Provider

**Files:**
- Modify: `src-tauri/src/provider/wireguard.rs`

- [ ] **Step 1: Write tests for WireGuardProvider**

Replace `src-tauri/src/provider/wireguard.rs` with:

```rust
use async_trait::async_trait;
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
    interface: String,
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
        todo!()
    }

    pub fn list_config_files() -> Vec<PathBuf> {
        todo!()
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

    async fn connect(&self, _opts: ConnectOptions) -> Result<(), VpnError> {
        todo!()
    }

    async fn disconnect(&self) -> Result<(), VpnError> {
        todo!()
    }

    async fn status(&self) -> Result<VpnStatus, VpnError> {
        todo!()
    }

    async fn get_config(&self) -> Result<ProviderConfig, VpnError> {
        todo!()
    }

    async fn set_config(&self, config: ProviderConfig) -> Result<(), VpnError> {
        todo!()
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
        let result = WireGuardProvider::parse_status("", "wg0");
        // Empty output means interface not active
        let status = result.unwrap();
        assert!(!status.connected);
    }

    #[test]
    fn test_list_config_files() {
        // This test just verifies the function doesn't panic
        let files = WireGuardProvider::list_config_files();
        // May or may not find files depending on system state
        for file in &files {
            assert!(file.extension().map(|e| e == "conf").unwrap_or(false));
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo test provider::wireguard::tests -- --nocapture
```

Expected: FAIL with `not yet implemented`.

- [ ] **Step 3: Implement parse_status and list_config_files**

Replace the `todo!()` in `parse_status`:

```rust
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
```

Replace the `todo!()` in `list_config_files`:

```rust
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
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo test provider::wireguard::tests -- --nocapture
```

Expected: All 4 tests PASS.

- [ ] **Step 5: Implement the VpnProvider trait methods**

Replace the `todo!()` implementations:

```rust
#[async_trait]
impl VpnProvider for WireGuardProvider {
    fn name(&self) -> &str {
        "WireGuard"
    }

    fn is_installed(&self) -> bool {
        is_tool_installed(TOOL_NAME)
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
        let cmd = format!("wg show {}", self.interface);
        match Self::exec_with_sudo(&cmd).await {
            Ok(output) => Self::parse_status(&output, &self.interface),
            Err(VpnError::CliError(_)) => {
                // Interface not active
                Ok(VpnStatus {
                    provider: "WireGuard".to_string(),
                    connected: false,
                    ip: None,
                    since: None,
                    latency_ms: None,
                    extra: {
                        let mut m = HashMap::new();
                        m.insert("interface".to_string(), self.interface.clone());
                        m
                    },
                })
            }
            Err(e) => Err(e),
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
        // WireGuard config changes require editing .conf files
        // For v1, we only support selecting which config file/interface to use
        // The actual interface switch happens on next connect
        Ok(())
    }
}
```

- [ ] **Step 6: Verify it compiles**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo check
```

Expected: Compiles successfully.

- [ ] **Step 7: Commit**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
git add src-tauri/src/provider/wireguard.rs
git commit -m "feat: implement WireGuard VPN provider with sudo support"
```

---

## Task 7: App State & Status Polling

**Files:**
- Create: `src-tauri/src/state.rs`
- Create: `src-tauri/src/settings.rs`

- [ ] **Step 1: Create settings module**

Create `src-tauri/src/settings.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub poll_interval_secs: u64,
    pub launch_at_login: bool,
    pub provider_visibility: HashMap<String, bool>,
}

impl Default for AppSettings {
    fn default() -> Self {
        let mut visibility = HashMap::new();
        visibility.insert("Tailscale".to_string(), true);
        visibility.insert("WARP".to_string(), true);
        visibility.insert("WireGuard".to_string(), true);

        Self {
            poll_interval_secs: 3,
            launch_at_login: false,
            provider_visibility: visibility,
        }
    }
}

impl AppSettings {
    pub fn load(path: &PathBuf) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &PathBuf) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }

    pub fn is_provider_visible(&self, name: &str) -> bool {
        self.provider_visibility
            .get(name)
            .copied()
            .unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_settings() {
        let settings = AppSettings::default();
        assert_eq!(settings.poll_interval_secs, 3);
        assert!(!settings.launch_at_login);
        assert!(settings.is_provider_visible("Tailscale"));
        assert!(settings.is_provider_visible("WARP"));
        assert!(settings.is_provider_visible("WireGuard"));
    }

    #[test]
    fn test_save_and_load() {
        let mut tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        let mut settings = AppSettings::default();
        settings.poll_interval_secs = 5;
        settings.save(&path).unwrap();

        let loaded = AppSettings::load(&path);
        assert_eq!(loaded.poll_interval_secs, 5);
    }

    #[test]
    fn test_load_missing_file() {
        let path = PathBuf::from("/tmp/nonexistent_conduit_test.json");
        let settings = AppSettings::load(&path);
        assert_eq!(settings.poll_interval_secs, 3); // defaults
    }
}
```

- [ ] **Step 2: Add tempfile dev-dependency to Cargo.toml**

Add to `src-tauri/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Run settings tests**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo test settings::tests -- --nocapture
```

Expected: All 3 tests PASS.

- [ ] **Step 4: Create state module**

Create `src-tauri/src/state.rs`:

```rust
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::provider::mod_prelude::*;
use crate::provider::tailscale::TailscaleProvider;
use crate::provider::warp::WarpProvider;
use crate::provider::wireguard::WireGuardProvider;
use crate::provider::{ProviderInfo, VpnError, VpnStatus};
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
```

- [ ] **Step 5: Add mod_prelude to provider/mod.rs**

Add at the bottom of `src-tauri/src/provider/mod.rs`:

```rust
pub mod mod_prelude {
    pub use super::VpnProvider;
}
```

- [ ] **Step 6: Register modules in main.rs**

Update `src-tauri/src/main.rs` to include:

```rust
mod provider;
mod settings;
mod state;
mod util;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 7: Verify it compiles**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo check
```

Expected: Compiles successfully.

- [ ] **Step 8: Commit**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
git add src-tauri/src/state.rs src-tauri/src/settings.rs
git commit -m "feat: add AppState with provider management and settings persistence"
```

---

## Task 8: Tauri IPC Commands

**Files:**
- Create: `src-tauri/src/commands.rs`

- [ ] **Step 1: Create commands module**

Create `src-tauri/src/commands.rs`:

```rust
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
        match result {
            Ok(status) => statuses.push(status),
            Err(_) => {} // Skip providers that error (not installed, etc.)
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
```

- [ ] **Step 2: Register commands in main.rs**

Update `src-tauri/src/main.rs`:

```rust
mod commands;
mod provider;
mod settings;
mod state;
mod util;

use std::sync::Arc;
use tokio::sync::Mutex;

fn main() {
    let settings_path = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("com.conduit.app")
        .join("settings.json");

    let app_state = Arc::new(Mutex::new(state::AppState::new(settings_path)));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::vpn_connect,
            commands::vpn_disconnect,
            commands::vpn_status,
            commands::vpn_status_all,
            commands::vpn_get_config,
            commands::vpn_set_config,
            commands::vpn_list_providers,
            commands::get_settings,
            commands::update_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Add dirs dependency to Cargo.toml**

Add to `src-tauri/Cargo.toml` dependencies:

```toml
dirs = "5"
```

- [ ] **Step 4: Verify it compiles**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo check
```

Expected: Compiles successfully.

- [ ] **Step 5: Commit**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
git add src-tauri/src/commands.rs src-tauri/src/main.rs src-tauri/Cargo.toml
git commit -m "feat: add Tauri IPC commands for VPN management and settings"
```

---

## Task 9: System Tray

**Files:**
- Create: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Create tray module**

Create `src-tauri/src/tray.rs`:

```rust
use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tokio::sync::Mutex;

use crate::provider::VpnStatus;
use crate::state::AppState;

pub fn create_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let quit = MenuItemBuilder::with_id("quit", "Quit Conduit").build(app)?;
    let open = MenuItemBuilder::with_id("open", "Open Dashboard").build(app)?;
    let separator = tauri::menu::PredefinedMenuItem::separator(app)?;

    let menu = MenuBuilder::new(app)
        .item(&open)
        .item(&separator)
        .item(&quit)
        .build()?;

    TrayIconBuilder::new()
        .icon(Image::from_bytes(include_bytes!("../icons/32x32.png"))?)
        .icon_as_template(true)
        .menu(&menu)
        .tooltip("Conduit - VPN Manager")
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "quit" => {
                app.exit(0);
            }
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

pub fn update_tray_menu(
    app: &AppHandle,
    statuses: &[VpnStatus],
) -> Result<(), Box<dyn std::error::Error>> {
    // Tray menu is rebuilt with current status info
    // In Tauri 2, we rebuild the menu items to reflect state
    let mut builder = MenuBuilder::new(app);

    for status in statuses {
        let dot = if status.connected { "●" } else { "○" };
        let state_text = if status.connected {
            "Connected"
        } else {
            "Disconnected"
        };
        let label = format!("{} {}  {}", dot, status.provider, state_text);

        let item_id = format!("toggle_{}", status.provider.to_lowercase());
        let item = MenuItemBuilder::with_id(&item_id, &label).build(app)?;
        builder = builder.item(&item);

        if status.connected {
            if let Some(ip) = &status.ip {
                let detail = MenuItemBuilder::with_id(
                    format!("detail_{}", status.provider.to_lowercase()),
                    format!("   {}",  ip),
                )
                .enabled(false)
                .build(app)?;
                builder = builder.item(&detail);
            }
        }

        builder = builder.item(&tauri::menu::PredefinedMenuItem::separator(app)?);
    }

    let open = MenuItemBuilder::with_id("open", "Open Dashboard").build(app)?;
    let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit Conduit").build(app)?;

    let menu = builder
        .item(&open)
        .item(&separator)
        .item(&quit)
        .build()?;

    // Update the tray icon's menu
    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(menu))?;
    }

    Ok(())
}
```

- [ ] **Step 2: Create polling loop and wire into main.rs**

Update `src-tauri/src/main.rs`:

```rust
mod commands;
mod provider;
mod settings;
mod state;
mod tray;
mod util;

use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

fn main() {
    let settings_path = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("com.conduit.app")
        .join("settings.json");

    let app_state = Arc::new(Mutex::new(state::AppState::new(settings_path)));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::vpn_connect,
            commands::vpn_disconnect,
            commands::vpn_status,
            commands::vpn_status_all,
            commands::vpn_get_config,
            commands::vpn_set_config,
            commands::vpn_list_providers,
            commands::get_settings,
            commands::update_settings,
        ])
        .setup(move |app| {
            // Create system tray
            tray::create_tray(app.handle())?;

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
                    let _ = tray::update_tray_menu(&app_handle, &statuses);
                }
            });

            // Keep app running when window is closed (menu bar app)
            #[cfg(target_os = "macos")]
            app.handle().plugin(tauri_plugin_opener::init())?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide instead of close
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Update tauri.conf.json for tray and window behavior**

The key settings to ensure in `src-tauri/tauri.conf.json`:

In the `app.windows` array, set:
```json
{
  "app": {
    "windows": [
      {
        "title": "Conduit",
        "width": 900,
        "height": 600,
        "visible": false
      }
    ],
    "trayIcon": {
      "id": "main",
      "iconPath": "icons/32x32.png",
      "iconAsTemplate": true
    }
  }
}
```

Set `visible: false` so the app starts hidden (tray-only). The window opens when user clicks "Open Dashboard."

- [ ] **Step 4: Verify it compiles**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo check
```

Expected: Compiles successfully.

- [ ] **Step 5: Commit**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
git add src-tauri/src/tray.rs src-tauri/src/main.rs src-tauri/tauri.conf.json
git commit -m "feat: add system tray with status polling and dashboard window management"
```

---

## Task 10: Frontend TypeScript Types & Tauri Bindings

**Files:**
- Create: `src/lib/types.ts`
- Create: `src/lib/tauri.ts`

- [ ] **Step 1: Create TypeScript types**

Create `src/lib/types.ts`:

```typescript
export interface VpnStatus {
  provider: string;
  connected: boolean;
  ip: string | null;
  since: string | null;
  latency_ms: number | null;
  extra: Record<string, string>;
}

export type ProviderConfig =
  | {
      type: "Tailscale";
      exit_node: string | null;
      accept_routes: boolean;
      shields_up: boolean;
    }
  | {
      type: "Warp";
      mode: WarpMode;
      families_mode: boolean;
    }
  | {
      type: "WireGuard";
      config_file: string;
      interface: string;
    };

export type WarpMode = "Warp" | "DnsOnly" | "Proxy";

export interface ConnectOptions {
  provider_config: ProviderConfig | null;
}

export interface ProviderInfo {
  name: string;
  installed: boolean;
  enabled: boolean;
}

export interface AppSettings {
  poll_interval_secs: number;
  launch_at_login: boolean;
  provider_visibility: Record<string, boolean>;
}

export interface ActivityEvent {
  timestamp: Date;
  provider: string;
  action: string;
  success: boolean;
  message?: string;
}
```

- [ ] **Step 2: Create Tauri IPC wrappers**

Create `src/lib/tauri.ts`:

```typescript
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  VpnStatus,
  ProviderConfig,
  ConnectOptions,
  ProviderInfo,
  AppSettings,
} from "./types";

export async function vpnConnect(
  provider: string,
  opts?: ConnectOptions,
): Promise<void> {
  return invoke("vpn_connect", { provider, opts });
}

export async function vpnDisconnect(provider: string): Promise<void> {
  return invoke("vpn_disconnect", { provider });
}

export async function vpnStatus(provider: string): Promise<VpnStatus> {
  return invoke("vpn_status", { provider });
}

export async function vpnStatusAll(): Promise<VpnStatus[]> {
  return invoke("vpn_status_all");
}

export async function vpnGetConfig(provider: string): Promise<ProviderConfig> {
  return invoke("vpn_get_config", { provider });
}

export async function vpnSetConfig(
  provider: string,
  config: ProviderConfig,
): Promise<void> {
  return invoke("vpn_set_config", { provider, config });
}

export async function vpnListProviders(): Promise<ProviderInfo[]> {
  return invoke("vpn_list_providers");
}

export async function getSettings(): Promise<AppSettings> {
  return invoke("get_settings");
}

export async function updateSettings(settings: AppSettings): Promise<void> {
  return invoke("update_settings", { settings });
}

export async function onStatusChanged(
  callback: (statuses: VpnStatus[]) => void,
): Promise<UnlistenFn> {
  return listen<VpnStatus[]>("vpn-status-changed", (event) => {
    callback(event.payload);
  });
}
```

- [ ] **Step 3: Verify frontend builds**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
npm run check 2>/dev/null || npx svelte-check --tsconfig ./tsconfig.json
```

Expected: No type errors.

- [ ] **Step 4: Commit**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
git add src/lib/types.ts src/lib/tauri.ts
git commit -m "feat: add TypeScript types and Tauri IPC bindings"
```

---

## Task 11: Frontend Svelte Stores

**Files:**
- Create: `src/lib/stores/vpn.ts`

- [ ] **Step 1: Create VPN store**

Create `src/lib/stores/vpn.ts`:

```typescript
import { writable, derived } from "svelte/store";
import type {
  VpnStatus,
  ProviderInfo,
  ActivityEvent,
  AppSettings,
} from "$lib/types";
import {
  vpnStatusAll,
  vpnListProviders,
  vpnConnect,
  vpnDisconnect,
  onStatusChanged,
  getSettings,
  updateSettings as updateSettingsApi,
} from "$lib/tauri";

export const statuses = writable<VpnStatus[]>([]);
export const providers = writable<ProviderInfo[]>([]);
export const activityLog = writable<ActivityEvent[]>([]);
export const settings = writable<AppSettings>({
  poll_interval_secs: 3,
  launch_at_login: false,
  provider_visibility: {},
});
export const isLoading = writable<Record<string, boolean>>({});

const MAX_LOG_ENTRIES = 50;

function addLogEntry(
  provider: string,
  action: string,
  success: boolean,
  message?: string,
) {
  activityLog.update((log) => {
    const entry: ActivityEvent = {
      timestamp: new Date(),
      provider,
      action,
      success,
      message,
    };
    const updated = [entry, ...log];
    return updated.slice(0, MAX_LOG_ENTRIES);
  });
}

export async function initialize() {
  try {
    const [providerList, statusList, currentSettings] = await Promise.all([
      vpnListProviders(),
      vpnStatusAll(),
      getSettings(),
    ]);
    providers.set(providerList);
    statuses.set(statusList);
    settings.set(currentSettings);

    // Subscribe to status changes from polling
    await onStatusChanged((newStatuses) => {
      statuses.set(newStatuses);
    });
  } catch (e) {
    console.error("Failed to initialize VPN stores:", e);
  }
}

export async function connect(provider: string) {
  isLoading.update((l) => ({ ...l, [provider]: true }));
  try {
    await vpnConnect(provider);
    addLogEntry(provider, "connected", true);
    // Fetch fresh status
    const fresh = await vpnStatusAll();
    statuses.set(fresh);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    addLogEntry(provider, "connect", false, msg);
  } finally {
    isLoading.update((l) => ({ ...l, [provider]: false }));
  }
}

export async function disconnect(provider: string) {
  isLoading.update((l) => ({ ...l, [provider]: true }));
  try {
    await vpnDisconnect(provider);
    addLogEntry(provider, "disconnected", true);
    const fresh = await vpnStatusAll();
    statuses.set(fresh);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    addLogEntry(provider, "disconnect", false, msg);
  } finally {
    isLoading.update((l) => ({ ...l, [provider]: false }));
  }
}

export async function saveSettings(newSettings: AppSettings) {
  try {
    await updateSettingsApi(newSettings);
    settings.set(newSettings);
  } catch (e) {
    console.error("Failed to save settings:", e);
  }
}

export const visibleStatuses = derived(
  [statuses, settings, providers],
  ([$statuses, $settings, $providers]) => {
    return $statuses.filter((s) => {
      const visible =
        $settings.provider_visibility[s.provider] !== false;
      return visible;
    });
  },
);
```

- [ ] **Step 2: Verify frontend builds**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
npm run check 2>/dev/null || npx svelte-check --tsconfig ./tsconfig.json
```

Expected: No type errors.

- [ ] **Step 3: Commit**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
git add src/lib/stores/vpn.ts
git commit -m "feat: add Svelte stores for VPN state management"
```

---

## Task 12: Frontend Components

**Files:**
- Create: `src/lib/components/StatusDot.svelte`
- Create: `src/lib/components/Toggle.svelte`
- Create: `src/lib/components/ConfigPanel.svelte`
- Create: `src/lib/components/ActivityLog.svelte`
- Create: `src/lib/components/VpnCard.svelte`

- [ ] **Step 1: Create StatusDot component**

Create `src/lib/components/StatusDot.svelte`:

```svelte
<script lang="ts">
  type Status = "connected" | "disconnected" | "connecting" | "error";

  let { status }: { status: Status } = $props();

  const colorMap: Record<Status, string> = {
    connected: "bg-green-500",
    disconnected: "bg-gray-400",
    connecting: "bg-yellow-400 animate-pulse",
    error: "bg-red-500",
  };
</script>

<span
  class="inline-block w-3 h-3 rounded-full {colorMap[status]}"
  title={status}
></span>
```

- [ ] **Step 2: Create Toggle component**

Create `src/lib/components/Toggle.svelte`:

```svelte
<script lang="ts">
  let {
    checked,
    disabled = false,
    onchange,
  }: {
    checked: boolean;
    disabled?: boolean;
    onchange: (checked: boolean) => void;
  } = $props();
</script>

<button
  role="switch"
  aria-checked={checked}
  {disabled}
  class="relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 {checked
    ? 'bg-green-500'
    : 'bg-gray-300'} {disabled ? 'opacity-50 cursor-not-allowed' : ''}"
  onclick={() => !disabled && onchange(!checked)}
>
  <span
    class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out {checked
      ? 'translate-x-5'
      : 'translate-x-0'}"
  ></span>
</button>
```

- [ ] **Step 3: Create ConfigPanel component**

Create `src/lib/components/ConfigPanel.svelte`:

```svelte
<script lang="ts">
  import type { ProviderConfig } from "$lib/types";
  import { vpnGetConfig, vpnSetConfig } from "$lib/tauri";

  let {
    provider,
    config,
  }: {
    provider: string;
    config: ProviderConfig | null;
  } = $props();

  let expanded = $state(false);
  let localConfig = $state<ProviderConfig | null>(config);

  async function loadConfig() {
    try {
      localConfig = await vpnGetConfig(provider);
    } catch (e) {
      console.error("Failed to load config:", e);
    }
  }

  async function saveConfig() {
    if (!localConfig) return;
    try {
      await vpnSetConfig(provider, localConfig);
    } catch (e) {
      console.error("Failed to save config:", e);
    }
  }

  function toggleExpanded() {
    expanded = !expanded;
    if (expanded && !localConfig) {
      loadConfig();
    }
  }
</script>

<div class="mt-3 border-t border-gray-200 dark:border-gray-700">
  <button
    class="flex items-center gap-1 w-full py-2 text-sm text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
    onclick={toggleExpanded}
  >
    <span class="text-xs transition-transform {expanded ? 'rotate-90' : ''}"
      >&#9654;</span
    >
    Configuration
  </button>

  {#if expanded && localConfig}
    <div class="pb-3 space-y-3 text-sm">
      {#if localConfig.type === "Tailscale"}
        <label class="flex items-center justify-between">
          <span class="text-gray-600 dark:text-gray-300">Exit Node</span>
          <input
            type="text"
            class="w-40 px-2 py-1 rounded border border-gray-300 dark:border-gray-600 dark:bg-gray-800 text-sm"
            value={localConfig.exit_node ?? ""}
            onchange={(e) => {
              if (localConfig?.type === "Tailscale") {
                localConfig.exit_node =
                  (e.target as HTMLInputElement).value || null;
                saveConfig();
              }
            }}
            placeholder="None"
          />
        </label>
        <label class="flex items-center justify-between">
          <span class="text-gray-600 dark:text-gray-300">Accept Routes</span>
          <input
            type="checkbox"
            class="rounded"
            checked={localConfig.accept_routes}
            onchange={() => {
              if (localConfig?.type === "Tailscale") {
                localConfig.accept_routes = !localConfig.accept_routes;
                saveConfig();
              }
            }}
          />
        </label>
        <label class="flex items-center justify-between">
          <span class="text-gray-600 dark:text-gray-300">Shields Up</span>
          <input
            type="checkbox"
            class="rounded"
            checked={localConfig.shields_up}
            onchange={() => {
              if (localConfig?.type === "Tailscale") {
                localConfig.shields_up = !localConfig.shields_up;
                saveConfig();
              }
            }}
          />
        </label>
      {:else if localConfig.type === "Warp"}
        <label class="flex items-center justify-between">
          <span class="text-gray-600 dark:text-gray-300">Mode</span>
          <select
            class="px-2 py-1 rounded border border-gray-300 dark:border-gray-600 dark:bg-gray-800 text-sm"
            value={localConfig.mode}
            onchange={(e) => {
              if (localConfig?.type === "Warp") {
                localConfig.mode = (e.target as HTMLSelectElement).value as any;
                saveConfig();
              }
            }}
          >
            <option value="Warp">WARP</option>
            <option value="DnsOnly">DNS Only</option>
            <option value="Proxy">Proxy</option>
          </select>
        </label>
        <label class="flex items-center justify-between">
          <span class="text-gray-600 dark:text-gray-300">Families Mode</span>
          <input
            type="checkbox"
            class="rounded"
            checked={localConfig.families_mode}
            onchange={() => {
              if (localConfig?.type === "Warp") {
                localConfig.families_mode = !localConfig.families_mode;
                saveConfig();
              }
            }}
          />
        </label>
      {:else if localConfig.type === "WireGuard"}
        <label class="flex items-center justify-between">
          <span class="text-gray-600 dark:text-gray-300">Interface</span>
          <input
            type="text"
            class="w-40 px-2 py-1 rounded border border-gray-300 dark:border-gray-600 dark:bg-gray-800 text-sm"
            value={localConfig.interface}
            disabled
          />
        </label>
        <label class="flex items-center justify-between">
          <span class="text-gray-600 dark:text-gray-300">Config File</span>
          <span class="text-xs text-gray-500 truncate max-w-[200px]">
            {localConfig.config_file}
          </span>
        </label>
      {/if}
    </div>
  {/if}
</div>
```

- [ ] **Step 4: Create ActivityLog component**

Create `src/lib/components/ActivityLog.svelte`:

```svelte
<script lang="ts">
  import { activityLog } from "$lib/stores/vpn";

  function formatTime(date: Date): string {
    return date.toLocaleTimeString("en-US", {
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    });
  }

  function clearLog() {
    activityLog.set([]);
  }
</script>

<div
  class="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-4"
>
  <div class="flex items-center justify-between mb-3">
    <h3 class="text-sm font-medium text-gray-700 dark:text-gray-300">
      Activity Log
    </h3>
    {#if $activityLog.length > 0}
      <button
        class="text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-200"
        onclick={clearLog}
      >
        Clear
      </button>
    {/if}
  </div>

  <div class="space-y-1 max-h-48 overflow-y-auto">
    {#if $activityLog.length === 0}
      <p class="text-sm text-gray-400 dark:text-gray-500 italic">
        No activity yet
      </p>
    {:else}
      {#each $activityLog as event}
        <div class="flex items-start gap-2 text-xs">
          <span class="text-gray-400 shrink-0 font-mono">
            {formatTime(event.timestamp)}
          </span>
          <span class={event.success ? "text-gray-600 dark:text-gray-300" : "text-red-500"}>
            {event.provider} {event.action}
            {#if event.message}
              <span class="text-gray-400"> — {event.message}</span>
            {/if}
          </span>
        </div>
      {/each}
    {/if}
  </div>
</div>
```

- [ ] **Step 5: Create VpnCard component**

Create `src/lib/components/VpnCard.svelte`:

```svelte
<script lang="ts">
  import type { VpnStatus, ProviderConfig } from "$lib/types";
  import StatusDot from "./StatusDot.svelte";
  import Toggle from "./Toggle.svelte";
  import ConfigPanel from "./ConfigPanel.svelte";
  import { connect, disconnect, isLoading } from "$lib/stores/vpn";

  let {
    status,
    installed = true,
  }: {
    status: VpnStatus | null;
    installed?: boolean;
  } = $props();

  let loading = $derived(
    status ? ($isLoading[status.provider] ?? false) : false,
  );
  let connected = $derived(status?.connected ?? false);
  let dotStatus = $derived<"connected" | "disconnected" | "connecting" | "error">(
    loading ? "connecting" : connected ? "connected" : "disconnected",
  );

  function handleToggle(checked: boolean) {
    if (!status) return;
    if (checked) {
      connect(status.provider);
    } else {
      disconnect(status.provider);
    }
  }

  function formatUptime(since: string | null): string {
    if (!since) return "";
    const start = new Date(since);
    const diff = Date.now() - start.getTime();
    const minutes = Math.floor(diff / 60000);
    const hours = Math.floor(minutes / 60);
    if (hours > 0) return `${hours}h ${minutes % 60}m`;
    return `${minutes}m`;
  }
</script>

<div
  class="rounded-xl border p-5 transition-colors {connected
    ? 'border-green-200 bg-green-50 dark:border-green-800 dark:bg-green-950'
    : 'border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-800'} {!installed
    ? 'opacity-50'
    : ''}"
>
  {#if !installed}
    <div class="text-center py-4">
      <p class="text-lg font-medium text-gray-400">
        {status?.provider ?? "Unknown"}
      </p>
      <p class="text-sm text-gray-400 mt-1">Not detected</p>
    </div>
  {:else if status}
    <div class="flex items-center justify-between mb-3">
      <div class="flex items-center gap-2">
        <StatusDot status={dotStatus} />
        <h2 class="text-lg font-semibold text-gray-800 dark:text-gray-100">
          {status.provider}
        </h2>
      </div>
      <Toggle
        checked={connected}
        disabled={loading}
        onchange={handleToggle}
      />
    </div>

    <div class="space-y-1 text-sm text-gray-600 dark:text-gray-400">
      <div class="flex justify-between">
        <span>Status</span>
        <span class={connected ? "text-green-600 dark:text-green-400" : ""}>
          {loading ? "Connecting..." : connected ? "Connected" : "Disconnected"}
        </span>
      </div>

      {#if status.ip}
        <div class="flex justify-between">
          <span>IP</span>
          <span class="font-mono text-xs">{status.ip}</span>
        </div>
      {/if}

      {#if status.since}
        <div class="flex justify-between">
          <span>Uptime</span>
          <span>{formatUptime(status.since)}</span>
        </div>
      {/if}

      {#each Object.entries(status.extra) as [key, value]}
        <div class="flex justify-between">
          <span class="capitalize">{key.replace(/_/g, " ")}</span>
          <span class="text-xs truncate max-w-[200px]">{value}</span>
        </div>
      {/each}
    </div>

    <ConfigPanel provider={status.provider} config={null} />
  {/if}
</div>
```

- [ ] **Step 6: Verify frontend builds**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
npm run check 2>/dev/null || npx svelte-check --tsconfig ./tsconfig.json
```

Expected: No type errors.

- [ ] **Step 7: Commit**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
git add src/lib/components/
git commit -m "feat: add VPN card, status dot, toggle, config panel, and activity log components"
```

---

## Task 13: Dashboard Page

**Files:**
- Modify: `src/routes/+page.svelte`

- [ ] **Step 1: Create the dashboard page**

Replace `src/routes/+page.svelte`:

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import VpnCard from "$lib/components/VpnCard.svelte";
  import ActivityLog from "$lib/components/ActivityLog.svelte";
  import {
    initialize,
    visibleStatuses,
    providers,
  } from "$lib/stores/vpn";

  let loaded = $state(false);

  onMount(async () => {
    await initialize();
    loaded = true;
  });
</script>

<div class="min-h-screen bg-gray-50 dark:bg-gray-900 p-6">
  <header class="mb-6">
    <h1 class="text-2xl font-bold text-gray-800 dark:text-gray-100">
      Conduit
    </h1>
    <p class="text-sm text-gray-500 dark:text-gray-400">
      Unified VPN Management
    </p>
  </header>

  {#if !loaded}
    <div class="flex items-center justify-center h-64">
      <p class="text-gray-400 animate-pulse">Loading...</p>
    </div>
  {:else}
    <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
      {#each $providers as provider}
        {@const status =
          $visibleStatuses.find((s) => s.provider === provider.name) ?? null}
        <VpnCard {status} installed={provider.installed} />
      {/each}
    </div>

    <ActivityLog />
  {/if}
</div>
```

- [ ] **Step 2: Verify frontend builds**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
npm run check 2>/dev/null || npx svelte-check --tsconfig ./tsconfig.json
```

Expected: No type errors.

- [ ] **Step 3: Commit**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
git add src/routes/+page.svelte
git commit -m "feat: add dashboard page with VPN cards grid and activity log"
```

---

## Task 14: Full Build & Smoke Test

**Files:**
- None (verification only)

- [ ] **Step 1: Run all Rust tests**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit/src-tauri
cargo test -- --nocapture
```

Expected: All unit tests PASS.

- [ ] **Step 2: Build the frontend**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
npm run build
```

Expected: SvelteKit builds successfully to `build/` directory.

- [ ] **Step 3: Build the full Tauri app**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
npm run tauri build -- --debug
```

Expected: Produces a `.app` bundle in `src-tauri/target/debug/bundle/macos/`.

- [ ] **Step 4: Run the dev server for manual testing**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
npm run tauri dev
```

Expected: App launches with a menu bar icon. Clicking the tray icon opens the dashboard. VPN cards show for installed tools (WARP and WireGuard based on your system). Toggle switches work.

- [ ] **Step 5: Verify menu bar tray**

Confirm:
- Conduit icon appears in macOS menu bar
- Clicking shows dropdown with VPN status items
- "Open Dashboard" opens the window
- Closing the window hides it (doesn't quit)
- "Quit Conduit" exits the app

- [ ] **Step 6: Commit any fixes**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
git add -A
git commit -m "fix: resolve build and integration issues from smoke test"
```

Only commit if fixes were needed. Skip this step if everything worked.

---

## Task 15: Settings UI

**Files:**
- Create: `src/lib/components/SettingsPanel.svelte`
- Modify: `src/routes/+page.svelte`

- [ ] **Step 1: Create SettingsPanel component**

Create `src/lib/components/SettingsPanel.svelte`:

```svelte
<script lang="ts">
  import { settings, saveSettings, providers } from "$lib/stores/vpn";
  import type { AppSettings } from "$lib/types";

  let { open, onclose }: { open: boolean; onclose: () => void } = $props();

  let localSettings = $state<AppSettings>({ ...$settings });

  $effect(() => {
    if (open) {
      localSettings = { ...$settings };
    }
  });

  async function handleSave() {
    await saveSettings(localSettings);
    onclose();
  }

  function updateVisibility(name: string, visible: boolean) {
    localSettings.provider_visibility = {
      ...localSettings.provider_visibility,
      [name]: visible,
    };
  }
</script>

{#if open}
  <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
    <div
      class="bg-white dark:bg-gray-800 rounded-xl p-6 w-96 shadow-xl"
    >
      <h2
        class="text-lg font-semibold text-gray-800 dark:text-gray-100 mb-4"
      >
        Settings
      </h2>

      <div class="space-y-4">
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            Poll Interval (seconds)
          </label>
          <input
            type="range"
            min="1"
            max="10"
            bind:value={localSettings.poll_interval_secs}
            class="w-full"
          />
          <span class="text-sm text-gray-500">{localSettings.poll_interval_secs}s</span>
        </div>

        <div>
          <label class="flex items-center justify-between text-sm">
            <span class="text-gray-700 dark:text-gray-300">Launch at Login</span>
            <input
              type="checkbox"
              bind:checked={localSettings.launch_at_login}
              class="rounded"
            />
          </label>
        </div>

        <div>
          <p class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Provider Visibility
          </p>
          {#each $providers as provider}
            <label class="flex items-center justify-between text-sm py-1">
              <span class="text-gray-600 dark:text-gray-400">{provider.name}</span>
              <input
                type="checkbox"
                checked={localSettings.provider_visibility[provider.name] !== false}
                onchange={(e) =>
                  updateVisibility(
                    provider.name,
                    (e.target as HTMLInputElement).checked,
                  )}
                class="rounded"
              />
            </label>
          {/each}
        </div>
      </div>

      <div class="flex justify-end gap-2 mt-6">
        <button
          class="px-4 py-2 text-sm text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200"
          onclick={onclose}
        >
          Cancel
        </button>
        <button
          class="px-4 py-2 text-sm bg-blue-500 text-white rounded-lg hover:bg-blue-600"
          onclick={handleSave}
        >
          Save
        </button>
      </div>
    </div>
  </div>
{/if}
```

- [ ] **Step 2: Add Settings button to dashboard page**

Update `src/routes/+page.svelte` to add a settings button in the header and the settings modal:

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import VpnCard from "$lib/components/VpnCard.svelte";
  import ActivityLog from "$lib/components/ActivityLog.svelte";
  import SettingsPanel from "$lib/components/SettingsPanel.svelte";
  import {
    initialize,
    visibleStatuses,
    providers,
  } from "$lib/stores/vpn";

  let loaded = $state(false);
  let settingsOpen = $state(false);

  onMount(async () => {
    await initialize();
    loaded = true;
  });
</script>

<div class="min-h-screen bg-gray-50 dark:bg-gray-900 p-6">
  <header class="flex items-center justify-between mb-6">
    <div>
      <h1 class="text-2xl font-bold text-gray-800 dark:text-gray-100">
        Conduit
      </h1>
      <p class="text-sm text-gray-500 dark:text-gray-400">
        Unified VPN Management
      </p>
    </div>
    <button
      class="p-2 text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 rounded-lg hover:bg-gray-200 dark:hover:bg-gray-700"
      onclick={() => (settingsOpen = true)}
      title="Settings"
    >
      <svg
        xmlns="http://www.w3.org/2000/svg"
        class="h-5 w-5"
        viewBox="0 0 20 20"
        fill="currentColor"
      >
        <path
          fill-rule="evenodd"
          d="M11.49 3.17c-.38-1.56-2.6-1.56-2.98 0a1.532 1.532 0 01-2.286.948c-1.372-.836-2.942.734-2.106 2.106.54.886.061 2.042-.947 2.287-1.561.379-1.561 2.6 0 2.978a1.532 1.532 0 01.947 2.287c-.836 1.372.734 2.942 2.106 2.106a1.532 1.532 0 012.287.947c.379 1.561 2.6 1.561 2.978 0a1.533 1.533 0 012.287-.947c1.372.836 2.942-.734 2.106-2.106a1.533 1.533 0 01.947-2.287c1.561-.379 1.561-2.6 0-2.978a1.532 1.532 0 01-.947-2.287c.836-1.372-.734-2.942-2.106-2.106a1.532 1.532 0 01-2.287-.947zM10 13a3 3 0 100-6 3 3 0 000 6z"
          clip-rule="evenodd"
        />
      </svg>
    </button>
  </header>

  {#if !loaded}
    <div class="flex items-center justify-center h-64">
      <p class="text-gray-400 animate-pulse">Loading...</p>
    </div>
  {:else}
    <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
      {#each $providers as provider}
        {@const status =
          $visibleStatuses.find((s) => s.provider === provider.name) ?? null}
        <VpnCard {status} installed={provider.installed} />
      {/each}
    </div>

    <ActivityLog />
  {/if}
</div>

<SettingsPanel open={settingsOpen} onclose={() => (settingsOpen = false)} />
```

- [ ] **Step 3: Verify frontend builds**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
npm run check 2>/dev/null || npx svelte-check --tsconfig ./tsconfig.json
```

Expected: No type errors.

- [ ] **Step 4: Commit**

```bash
cd /Users/cs/Workspaces/cchitsiang/conduit
git add src/lib/components/SettingsPanel.svelte src/routes/+page.svelte
git commit -m "feat: add settings panel with provider visibility and poll interval controls"
```

---

## Summary

| Task | What it builds | Key files |
|------|---------------|-----------|
| 1 | Project scaffolding | Tauri 2 + SvelteKit + Tailwind |
| 2 | CLI execution utility | `util/exec.rs`, `util/detect.rs` |
| 3 | VpnProvider trait & types | `provider/mod.rs` |
| 4 | Tailscale provider | `provider/tailscale.rs` |
| 5 | WARP provider | `provider/warp.rs` |
| 6 | WireGuard provider | `provider/wireguard.rs` |
| 7 | App state & settings | `state.rs`, `settings.rs` |
| 8 | Tauri IPC commands | `commands.rs` |
| 9 | System tray + polling | `tray.rs`, `main.rs` |
| 10 | Frontend TS bindings | `lib/types.ts`, `lib/tauri.ts` |
| 11 | Svelte stores | `lib/stores/vpn.ts` |
| 12 | UI components | 5 Svelte components |
| 13 | Dashboard page | `+page.svelte` |
| 14 | Build & smoke test | Verification |
| 15 | Settings UI | `SettingsPanel.svelte`, `+page.svelte` |
