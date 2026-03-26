<script lang="ts">
  import type { ProviderConfig } from "$lib/types";
  import { vpnGetConfig, vpnSetConfig, listWireguardConfigs, importWireguardConfig, listPritunlProfiles, type WgConfigInfo, type PritunlProfileInfo } from "$lib/tauri";
  import { open } from "@tauri-apps/plugin-dialog";

  let {
    provider,
    config,
  }: {
    provider: string;
    config: ProviderConfig | null;
  } = $props();

  let expanded = $state(false);
  let localConfig = $state<ProviderConfig | null>(null);
  let wgConfigs = $state<WgConfigInfo[]>([]);
  let pritunlProfiles = $state<PritunlProfileInfo[]>([]);

  $effect(() => {
    localConfig = config;
  });

  async function loadConfig() {
    try {
      localConfig = await vpnGetConfig(provider);
      if (provider === "WireGuard") {
        wgConfigs = await listWireguardConfigs();
      }
      if (provider === "Pritunl") {
        pritunlProfiles = await listPritunlProfiles();
      }
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
            class="w-40 px-2 py-1 rounded border border-gray-300 dark:border-gray-600 dark:bg-gray-800 dark:text-white text-sm"
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
          <span class="text-gray-600 dark:text-gray-300">Profile</span>
          <select
            class="w-40 px-2 py-1 rounded border border-gray-300 dark:border-gray-600 dark:bg-gray-800 dark:text-white text-sm"
            value={localConfig.interface}
            onchange={async (e) => {
              if (localConfig?.type === "WireGuard") {
                const selected = (e.target as HTMLSelectElement).value;
                if (selected === "__add_profile__") {
                  // Reset dropdown to current value
                  (e.target as HTMLSelectElement).value = localConfig.interface;
                  const file = await open({
                    title: "Select WireGuard Config",
                    filters: [{ name: "WireGuard Config", extensions: ["conf"] }],
                    multiple: false,
                  });
                  if (file) {
                    try {
                      const imported = await importWireguardConfig(file);
                      wgConfigs = await listWireguardConfigs();
                      localConfig.interface = imported.name;
                      localConfig.config_file = imported.path;
                      saveConfig();
                    } catch (err) {
                      console.error("Failed to import config:", err);
                    }
                  }
                  return;
                }
                const cfg = wgConfigs.find((c) => c.name === selected);
                localConfig.interface = selected;
                localConfig.config_file = cfg?.path ?? `/etc/wireguard/${selected}.conf`;
                saveConfig();
              }
            }}
          >
            {#each wgConfigs as cfg}
              <option value={cfg.name}>{cfg.name}</option>
            {/each}
            {#if !wgConfigs.find((c) => c.name === localConfig.interface)}
              <option value={localConfig.interface}>{localConfig.interface}</option>
            {/if}
            <option disabled>---</option>
            <option value="__add_profile__">Add Profile...</option>
          </select>
        </label>
        <div class="flex items-center justify-between">
          <span class="text-gray-600 dark:text-gray-300">Config File</span>
          <span class="text-xs text-gray-500 truncate max-w-[200px]">
            {localConfig.config_file}
          </span>
        </div>
      {:else if localConfig.type === "Pritunl"}
        <label class="flex items-center justify-between">
          <span class="text-gray-600 dark:text-gray-300">Profile</span>
          <select
            class="w-40 px-2 py-1 rounded border border-gray-300 dark:border-gray-600 dark:bg-gray-800 dark:text-white text-sm"
            value={localConfig.profile_id}
            onchange={(e) => {
              if (localConfig?.type === "Pritunl") {
                const selected = (e.target as HTMLSelectElement).value;
                localConfig.profile_id = selected;
                localConfig.password = null;
                saveConfig();
              }
            }}
          >
            {#each pritunlProfiles as profile}
              <option value={profile.id}>{profile.name}</option>
            {/each}
            {#if localConfig.profile_id && !pritunlProfiles.find((p) => p.id === localConfig.profile_id)}
              <option value={localConfig.profile_id}>{localConfig.profile_id}</option>
            {/if}
          </select>
        </label>
        {@const selectedProfile = pritunlProfiles.find((p) => p.id === localConfig.profile_id)}
        {#if selectedProfile?.user}
          <div class="flex items-center justify-between">
            <span class="text-gray-600 dark:text-gray-300">User</span>
            <span class="text-xs text-gray-500 truncate max-w-[200px]">{selectedProfile.user}</span>
          </div>
        {/if}
        {#if selectedProfile?.password_mode}
          <div class="flex items-center justify-between">
            <span class="text-gray-600 dark:text-gray-300">Auth</span>
            <span class="text-xs text-gray-500 capitalize">{selectedProfile.password_mode}</span>
          </div>
        {/if}
      {/if}
    </div>
  {/if}
</div>
