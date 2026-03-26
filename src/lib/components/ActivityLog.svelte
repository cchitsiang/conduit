<script lang="ts">
  import { activityLog } from "$lib/stores/vpn";

  function formatTime(date: Date): string {
    return date.toLocaleTimeString("en-US", {
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    });
  }

  function clearLog() {
    activityLog.set([]);
  }
</script>

<div
  class="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-4"
>
  <div class="flex items-center justify-between mb-3">
    <h3 class="text-sm font-medium text-gray-700 dark:text-gray-300">
      Activity Log
    </h3>
    {#if $activityLog.length > 0}
      <button
        class="text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-200"
        onclick={clearLog}
      >
        Clear
      </button>
    {/if}
  </div>

  <div class="space-y-1 max-h-48 overflow-y-auto">
    {#if $activityLog.length === 0}
      <p class="text-sm text-gray-400 dark:text-gray-500 italic">
        No activity yet
      </p>
    {:else}
      {#each $activityLog as event}
        <div class="flex items-start gap-2 text-xs">
          <span class="text-gray-400 shrink-0 font-mono">
            {formatTime(event.timestamp)}
          </span>
          <span class={event.success ? "text-gray-600 dark:text-gray-300" : "text-red-500"}>
            {event.provider} {event.action}
            {#if event.message}
              <span class="text-gray-400"> — {event.message}</span>
            {/if}
          </span>
        </div>
      {/each}
    {/if}
  </div>
</div>
