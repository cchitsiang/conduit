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
