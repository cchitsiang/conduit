# Conduit - Unified VPN Management App

**Date:** 2026-03-26
**Status:** Draft

## Overview

Conduit is a macOS desktop app that provides a single interface to manage connect/disconnect status, view connection details, and configure three VPN tools: Tailscale, Cloudflare WARP (Zero Trust), and WireGuard.

## Goals

- One app to toggle and monitor all three VPN connections
- Menu bar tray for quick access, full dashboard window for details and configuration
- Detect installed tools automatically, gracefully handle missing ones
- Allow simultaneous VPN connections (user decides)
- Start with CLI wrappers for simplicity, upgrade to native APIs later if needed

## Non-Goals

- Mobile or cross-platform support (macOS only for v1)
- Managing VPN accounts or authentication (users set up accounts via each tool's own flow)
- Auto-reconnect rules or scheduling (v1 is manual control only)
- Persisting activity logs to disk

## Tech Stack

| Layer     | Technology                              |
|-----------|-----------------------------------------|
| Framework | Tauri 2.x                               |
| Backend   | Rust, Tokio, Serde, async-trait, chrono |
| Frontend  | SvelteKit 5, TypeScript, Tailwind CSS 4 |
| Build     | Vite, @tauri-apps/cli                   |
| Target    | macOS (Apple Silicon + Intel)           |

## Architecture

```
macOS Menu Bar Tray
        │
        ▼
SvelteKit Frontend (Tauri WebView)
        │ Tauri IPC (invoke / events)
        ▼
Rust Backend
  ├── VpnProvider trait
  ├── tailscale.rs  → shells out to `tailscale` CLI
  ├── warp.rs       → shells out to `warp-cli`
  └── wireguard.rs  → shells out to `wg` / `wg-quick`
```

Single Tauri process. Rust backend owns all VPN interactions through a shared `VpnProvider` trait. Each VPN tool gets its own module implementing the trait. A Tokio background task polls status every 3 seconds and pushes updates to the frontend via Tauri events.

## VpnProvider Trait & Data Model

### Core Trait

```rust
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

### Shared Types

```rust
pub struct VpnStatus {
    pub provider: String,
    pub connected: bool,
    pub ip: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub latency_ms: Option<u32>,
    pub extra: HashMap<String, String>,
}

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

pub enum WarpMode { Warp, DnsOnly, Proxy }

pub struct ConnectOptions {
    pub provider_config: Option<ProviderConfig>,
}

pub enum VpnError {
    NotInstalled,
    CliError(String),
    ParseError(String),
    PermissionDenied,
    Timeout,
}
```

### Provider-Specific Extra Fields

- **Tailscale:** `tailnet_name`, `hostname`, `exit_node`, `peers_count`
- **WARP:** `warp_mode`, `account_type` (free/teams/zero_trust), `gateway`
- **WireGuard:** `interface`, `endpoint`, `transfer_rx`, `transfer_tx`, `latest_handshake`

## CLI Integration

### Tailscale

| Action            | Command                                  |
|-------------------|------------------------------------------|
| Detect installed  | `which tailscale`                        |
| Connect           | `tailscale up`                           |
| Connect w/ exit   | `tailscale up --exit-node=<node>`        |
| Disconnect        | `tailscale down`                         |
| Status            | `tailscale status --json`                |
| List exit nodes   | `tailscale exit-node list --json`        |
| Config changes    | `tailscale set --accept-routes --shields-up` |

### WARP

| Action            | Command                                  |
|-------------------|------------------------------------------|
| Detect installed  | `which warp-cli`                         |
| Connect           | `warp-cli connect`                       |
| Disconnect        | `warp-cli disconnect`                    |
| Status            | `warp-cli status`                        |
| Set mode          | `warp-cli mode warp` / `doh` / `proxy`  |
| Families mode     | `warp-cli dns families malware` / `off`  |
| Account info      | `warp-cli account`                       |

### WireGuard

| Action            | Command                                  |
|-------------------|------------------------------------------|
| Detect installed  | `which wg`                               |
| Connect           | `wg-quick up <interface>` (sudo)         |
| Disconnect        | `wg-quick down <interface>` (sudo)       |
| Status            | `wg show <interface>` (sudo)             |
| List configs      | Scan `/etc/wireguard/*.conf` and `/opt/homebrew/etc/wireguard/*.conf` |

### Sudo Handling (WireGuard)

Uses `osascript -e 'do shell script "..." with administrator privileges'` to trigger macOS native privilege prompt. No password storage. User cancellation returns `VpnError::PermissionDenied`.

### CLI Execution Pattern

All CLI calls go through a shared `exec_command` utility:
- Runs via `tokio::process::Command`
- 10-second timeout
- Captures stdout + stderr
- Returns `Result<String, VpnError>`

## UI Design

### Menu Bar Tray

```
┌──────────────────────────────┐
│  ● Tailscale    Connected    │  green dot
│    192.168.1.5  ·  2h 14m    │
│  ─────────────────────────── │
│  ○ WARP         Disconnected │  grey dot
│  ─────────────────────────── │
│  ● WireGuard   Connected    │  green dot
│    wg0  ·  45m               │
│  ─────────────────────────── │
│  ◆ Open Dashboard            │
│  ─────────────────────────── │
│  Settings                    │
│  Quit Conduit                │
└──────────────────────────────┘
```

- Each VPN row is clickable to toggle connect/disconnect
- Status dots: green (connected), grey (disconnected), yellow (connecting), red (error)
- Shows IP and uptime when connected

### Dashboard Window

Three-card layout with one card per VPN provider.

**Card states:**
- **Connected:** green accent, status details, config expander
- **Disconnected:** muted/grey, toggle + last known config
- **Not installed:** dimmed with "Not detected" message, option to hide
- **Connecting:** pulsing yellow accent with spinner
- **Error:** red accent with error message

**Config expanders per provider:**
- **Tailscale:** exit node dropdown, accept routes toggle, shields up toggle
- **WARP:** mode selector (Warp/DNS-only/Proxy), families mode toggle
- **WireGuard:** config file selector dropdown, interface name

**Activity Log:** Rolling list of the last 50 connection events with timestamps, stored in memory only.

**App lifecycle:** Closing the dashboard keeps the app in the menu bar. Quit from tray exits cleanly but does NOT disconnect active VPNs.

## Tauri IPC Commands

```typescript
// Frontend → Rust
invoke('vpn_connect', { provider: string, opts?: ConnectOptions })
invoke('vpn_disconnect', { provider: string })
invoke('vpn_status', { provider: string }) → VpnStatus
invoke('vpn_status_all') → VpnStatus[]
invoke('vpn_get_config', { provider: string }) → ProviderConfig
invoke('vpn_set_config', { provider: string, config: ProviderConfig })
invoke('vpn_list_providers') → ProviderInfo[]

// Rust → Frontend (events)
listen('vpn-status-changed', (payload: VpnStatus[]))
```

## Polling & Event Flow

1. On app start, Rust spawns a Tokio task polling all providers every 3 seconds
2. On status change, emits `vpn-status-changed` event to frontend
3. Frontend Svelte store subscribes, reactively updates cards and tray
4. User actions (toggle, config) call `invoke()`, execute immediately, trigger re-poll

## Error Handling

- **CLI timeout:** 10-second limit, shows "Timed out" in card
- **CLI failure:** stderr captured, surfaced in activity log and card error state. Known patterns parsed for actionable messages (e.g. "not logged in")
- **Tool not found:** Detection on launch, cached. Re-detected on poll failure. Users can trigger re-detection from Settings
- **Sudo cancel (WireGuard):** returns `PermissionDenied`, shows "Admin permission required"
- **Concurrent ops:** `tokio::sync::Mutex` per provider prevents overlapping connect/disconnect calls
- **Stale state:** Polling detects external changes (e.g. user runs CLI directly). Frontend always reflects actual CLI state

## Settings

Accessible from the tray menu. Minimal for v1:

- **Provider visibility:** toggle which VPN cards appear in the dashboard and tray (for tools the user doesn't want to manage)
- **Poll interval:** adjust status polling frequency (default 3 seconds, range 1-10)
- **Launch at login:** toggle macOS login item
- **Re-detect tools:** manually trigger tool detection scan

Settings are persisted to a JSON file at `~/Library/Application Support/com.conduit.app/settings.json` via Tauri's `app_data_dir`.

## Project Structure

```
conduit/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── icons/
│   ├── src/
│   │   ├── main.rs              # Tauri entry, setup tray + window
│   │   ├── tray.rs              # Menu bar tray construction + events
│   │   ├── commands.rs          # Tauri IPC command handlers
│   │   ├── state.rs             # AppState: providers, poll loop
│   │   ├── provider/
│   │   │   ├── mod.rs           # VpnProvider trait + shared types
│   │   │   ├── tailscale.rs     # Tailscale CLI wrapper
│   │   │   ├── warp.rs          # WARP CLI wrapper
│   │   │   └── wireguard.rs     # WireGuard CLI wrapper
│   │   └── util/
│   │       ├── exec.rs          # CLI exec helper with timeout
│   │       └── detect.rs        # Tool detection
├── src/
│   ├── app.html
│   ├── routes/
│   │   └── +page.svelte         # Dashboard main view
│   ├── lib/
│   │   ├── components/
│   │   │   ├── VpnCard.svelte
│   │   │   ├── StatusDot.svelte
│   │   │   ├── ConfigPanel.svelte
│   │   │   ├── ActivityLog.svelte
│   │   │   └── Toggle.svelte
│   │   ├── stores/
│   │   │   └── vpn.ts
│   │   └── tauri.ts
│   └── app.css
├── static/
├── svelte.config.js
├── tailwind.config.js
├── tsconfig.json
├── package.json
└── docs/
```
