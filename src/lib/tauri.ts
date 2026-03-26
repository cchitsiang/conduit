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
