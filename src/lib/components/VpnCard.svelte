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
  let pendingAction = $state<"connect" | "disconnect" | null>(null);
  let dotStatus = $derived<"connected" | "disconnected" | "connecting" | "error">(
    loading ? "connecting" : connected ? "connected" : "disconnected",
  );

  let showOtpPrompt = $state(false);
  let otpValue = $state("");

  // Check if current Pritunl profile needs OTP/password
  let needsOtp = $derived(
    name === "Pritunl" && !!status?.extra?.["password_mode"],
  );

  // Clear pendingAction when loading finishes
  $effect(() => {
    if (!loading) pendingAction = null;
  });

  function handleToggle(checked: boolean) {
    if (checked) {
      pendingAction = "connect";
      if (name === "Pritunl" && needsOtp) {
        showOtpPrompt = true;
        otpValue = "";
        return;
      }
      connect(name);
    } else {
      pendingAction = "disconnect";
      disconnect(name);
    }
  }

  function submitOtp() {
    showOtpPrompt = false;
    pendingAction = "connect";
    connect(name, {
      provider_config: {
        type: "Pritunl",
        profile_id: status?.extra?.["profile"] ?? "",
        password: otpValue,
      },
    });
    otpValue = "";
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
          {#if loading && pendingAction === "disconnect"}
            Disconnecting...
          {:else if loading}
            Connecting...
          {:else if connected}
            Connected
          {:else}
            Disconnected
          {/if}
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

    {#if showOtpPrompt}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
        onkeydown={(e) => { if (e.key === "Escape") showOtpPrompt = false; }}
        onclick={(e) => { if (e.target === e.currentTarget) showOtpPrompt = false; }}
      >
        <div class="bg-white dark:bg-gray-800 rounded-xl shadow-2xl p-6 w-80">
          <h3 class="text-base font-semibold text-gray-800 dark:text-gray-100 mb-1">
            {status?.extra?.["password_mode"] === "pin" ? "Enter PIN" : "Enter OTP Code"}
          </h3>
          <p class="text-xs text-gray-500 dark:text-gray-400 mb-4">
            {status?.extra?.["server"] ?? "Pritunl"} requires authentication
          </p>
          <form onsubmit={(e) => { e.preventDefault(); submitOtp(); }}>
            <input
              type="text"
              class="w-full px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 dark:bg-gray-700 text-gray-900 dark:text-white text-center tracking-widest text-lg font-mono focus:outline-none focus:ring-2 focus:ring-green-500"
              placeholder="000000"
              bind:value={otpValue}
              autofocus
            />
            <div class="flex gap-2 mt-4">
              <button
                type="submit"
                class="flex-1 py-2 text-sm font-medium rounded-lg bg-green-500 text-white hover:bg-green-600 transition-colors"
              >
                Connect
              </button>
              <button
                type="button"
                class="flex-1 py-2 text-sm font-medium rounded-lg border border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
                onclick={() => (showOtpPrompt = false)}
              >
                Cancel
              </button>
            </div>
          </form>
        </div>
      </div>
    {/if}

    <ConfigPanel provider={name} config={null} />
  {/if}
</div>
