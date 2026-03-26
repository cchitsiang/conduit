# Conduit

Unified macOS menu bar app to manage **Tailscale**, **Cloudflare WARP**, and **WireGuard** VPN connections from a single interface.

![Conduit Dashboard](assets/screenshot-dashboard.png)

## Features

- **Menu bar tray** — quick status overview and toggles without opening a window
- **Dashboard window** — detailed cards per VPN with status, IP, uptime, and configuration
- **Connect / Disconnect** — toggle any VPN with one click
- **Status polling** — automatically detects connection changes every 3 seconds
- **Configuration management**
  - Tailscale: exit node, accept routes, shields up
  - WARP: mode (WARP/DNS-only/Proxy), families mode
  - WireGuard: profile selector from `~/.conduit/wireguard/` configs
- **Activity log** — rolling log of connect/disconnect events
- **Settings** — poll interval, provider visibility, launch at login

## Tech Stack

| Layer     | Technology                     |
|-----------|--------------------------------|
| Framework | Tauri 2.x                      |
| Backend   | Rust, Tokio, Serde, async-trait|
| Frontend  | SvelteKit 5, TypeScript        |
| Styling   | Tailwind CSS 4                 |
| Target    | macOS (Apple Silicon + Intel)  |

## Prerequisites

- [Rust](https://rustup.rs/) (stable 1.88+)
- [Node.js](https://nodejs.org/) (20+)
- At least one of the following VPN tools:
  - **Tailscale** — [Mac App](https://tailscale.com/download/mac) or `brew install tailscale`
  - **Cloudflare WARP** — [Download](https://1.1.1.1/)
  - **WireGuard** — `brew install wireguard-tools`

## Getting Started

```bash
# Clone
git clone https://github.com/cchitsiang/conduit.git
cd conduit

# Install dependencies
npm install

# Run in development
npm run tauri dev

# Build for production
npm run tauri build
```

## Installation from DMG

Download the latest `.dmg` from [Releases](https://github.com/cchitsiang/conduit/releases), open it, and drag Conduit to Applications.

Since the app is not code-signed, macOS will quarantine it. Run this after installing:

```bash
xattr -cr /Applications/conduit.app
```

Then open Conduit from Applications or Spotlight.

## WireGuard Profiles

Place your WireGuard `.conf` files in `~/.conduit/wireguard/`:

```bash
mkdir -p ~/.conduit/wireguard
cp /path/to/your-profile.conf ~/.conduit/wireguard/
```

The app will scan this directory and show available profiles in the WireGuard configuration dropdown. System directories (`/etc/wireguard/`, `/opt/homebrew/etc/wireguard/`, `/usr/local/etc/wireguard/`) are also checked as fallback.

## Architecture

```
macOS Menu Bar Tray
        |
        v
SvelteKit Frontend (Tauri WebView)
        | Tauri IPC (invoke / events)
        v
Rust Backend
  |-- VpnProvider trait
  |-- tailscale.rs  -> Tailscale Mac App CLI or standalone CLI
  |-- warp.rs       -> warp-cli
  |-- wireguard.rs  -> wg / wg-quick (sudo via osascript)
```

Single Tauri process. Each VPN tool gets a Rust module implementing the `VpnProvider` trait. A Tokio background task polls status and pushes updates to the frontend via Tauri events.

## Project Structure

```
conduit/
├── src-tauri/src/
│   ├── main.rs              # App entry, tray, polling loop
│   ├── commands.rs          # Tauri IPC command handlers
│   ├── state.rs             # AppState: providers + settings
│   ├── settings.rs          # JSON-persisted settings
│   ├── tray.rs              # macOS menu bar tray
│   ├── provider/
│   │   ├── mod.rs           # VpnProvider trait + shared types
│   │   ├── tailscale.rs     # Tailscale CLI wrapper
│   │   ├── warp.rs          # WARP CLI wrapper
│   │   └── wireguard.rs     # WireGuard CLI wrapper
│   └── util/
│       ├── exec.rs          # Async CLI runner with timeout
│       └── detect.rs        # Tool detection
├── src/
│   ├── routes/+page.svelte  # Dashboard page
│   └── lib/
│       ├── components/      # Svelte components
│       ├── stores/vpn.ts    # Reactive state management
│       ├── tauri.ts         # Typed IPC wrappers
│       └── types.ts         # TypeScript types
└── docs/                    # Design spec + implementation plan
```

## License

MIT
