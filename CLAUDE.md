# CLAUDE.md

## Project Overview

Conduit is a macOS menu bar + dashboard app for managing Tailscale, Cloudflare WARP, and WireGuard VPN connections. Built with Tauri 2 (Rust backend) + SvelteKit 5 (frontend).

## Build & Run

```bash
npm install          # Install frontend dependencies
npm run tauri dev    # Dev mode with hot reload
npm run tauri build  # Production build (.app bundle)
```

## Test

```bash
cd src-tauri && cargo test   # 22 Rust unit tests
npm run build                # Verify frontend builds
```

## Architecture

- **Rust backend** (`src-tauri/src/`): `VpnProvider` trait implemented by three providers (tailscale.rs, warp.rs, wireguard.rs). All VPN interactions go through CLI wrappers via `util/exec.rs`. State managed in `state.rs`, IPC in `commands.rs`.
- **Frontend** (`src/`): SvelteKit 5 with Svelte runes ($state, $derived, $props). Stores in `lib/stores/vpn.ts`. Components in `lib/components/`.
- **IPC**: Frontend calls Rust via `invoke()`, Rust pushes status updates via `emit("vpn-status-changed")`.

## Key Patterns

- Lib crate is named `conduit_lib` (see Cargo.toml `[lib]`). In `main.rs`, reference modules via `conduit_lib::`.
- Tailscale uses Mac App CLI at `/Applications/Tailscale.app/Contents/MacOS/Tailscale` when available, falls back to standalone `tailscale`.
- WireGuard uses `osascript` for sudo (connect/disconnect only). Status polling uses `/var/run/wireguard/<name>.name` file existence check (no sudo).
- WireGuard configs stored in `~/.conduit/wireguard/` (user-writable, no sudo to read).
- `VpnStatus.extra` uses `BTreeMap` (not HashMap) for stable field ordering in the UI.
- Window hides on close (stays in menu bar). Dock click triggers `RunEvent::Reopen` to re-show.

## Code Style

- Rust: standard formatting, async-trait for providers, Tokio for async
- Frontend: Svelte 5 runes syntax, Tailwind CSS 4, TypeScript strict
- No emojis in code or comments
