<script lang="ts">
  import type { VpnStatus, ProviderConfig } from "$lib/types";
  import StatusDot from "./StatusDot.svelte";
  import Toggle from "./Toggle.svelte";
  import ConfigPanel from "./ConfigPanel.svelte";
  import { connect, disconnect, isLoading } from "$lib/stores/vpn";

  let {
    name,
    status,
    installed = true,
  }: {
    name: string;
    status: VpnStatus | null;
    installed?: boolean;
  } = $props();

  let loading = $derived($isLoading[name] ?? false);
  let connected = $derived(status?.connected ?? false);
  let dotStatus = $derived<"connected" | "disconnected" | "connecting" | "error">(
    loading ? "connecting" : connected ? "connected" : "disconnected",
  );

  function handleToggle(checked: boolean) {
    if (checked) {
      connect(name);
    } else {
      disconnect(name);
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
        {name}
      </p>
      <p class="text-sm text-gray-400 mt-1">Not detected</p>
      <p class="text-xs text-gray-500 mt-3">
        {#if name === "Tailscale"}
          Install via: <code class="bg-gray-700 px-1 rounded">brew install tailscale</code>
        {:else if name === "WARP"}
          Install <a href="https://1.1.1.1/" class="text-blue-400 underline" target="_blank">Cloudflare WARP</a> from 1.1.1.1
        {:else if name === "WireGuard"}
          Install via: <code class="bg-gray-700 px-1 rounded">brew install wireguard-tools</code>
        {/if}
      </p>
    </div>
  {:else}
    <div class="flex items-center justify-between mb-3">
      <div class="flex items-center gap-2">
        <StatusDot status={dotStatus} />
        <h2 class="text-lg font-semibold text-gray-800 dark:text-gray-100">
          {name}
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

      {#if status?.ip}
        <div class="flex justify-between">
          <span>IP</span>
          <span class="font-mono text-xs">{status.ip}</span>
        </div>
      {/if}

      {#if status?.since}
        <div class="flex justify-between">
          <span>Uptime</span>
          <span>{formatUptime(status.since)}</span>
        </div>
      {/if}

      {#if status}
        {#each Object.entries(status.extra) as [key, value]}
          <div class="flex justify-between">
            <span class="capitalize">{key.replace(/_/g, " ")}</span>
            <span class="text-xs truncate max-w-[200px]">{value}</span>
          </div>
        {/each}
      {/if}
    </div>

    <ConfigPanel provider={name} config={null} />
  {/if}
</div>
