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
  let localConfig = $state<ProviderConfig | null>(null);
  $effect(() => {
    localConfig = config;
  });

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
        <div class="flex items-center justify-between">
          <span class="text-gray-600 dark:text-gray-300">Config File</span>
          <span class="text-xs text-gray-500 truncate max-w-[200px]">
            {localConfig.config_file}
          </span>
        </div>
      {/if}
    </div>
  {/if}
</div>
