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
    // Check actual status after connect command returns
    const fresh = await vpnStatusAll();
    statuses.set(fresh);
    const actual = fresh.find((s) => s.provider === provider);
    if (actual?.connected) {
      addLogEntry(provider, "connected", true);
    } else {
      addLogEntry(provider, "connecting", true, "command sent, waiting for connection");
    }
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
    const fresh = await vpnStatusAll();
    statuses.set(fresh);
    const actual = fresh.find((s) => s.provider === provider);
    if (!actual?.connected) {
      addLogEntry(provider, "disconnected", true);
    } else {
      addLogEntry(provider, "disconnecting", true, "command sent, waiting for disconnection");
    }
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
