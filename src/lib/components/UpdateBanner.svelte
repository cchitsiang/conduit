<script lang="ts">
  import { onMount } from "svelte";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";

  let update = $state<Update | null>(null);
  let updating = $state(false);
  let progress = $state("");
  let dismissed = $state(false);

  onMount(async () => {
    try {
      update = await check();
    } catch (e) {
      console.error("Update check failed:", e);
    }
  });

  async function installUpdate() {
    if (!update) return;
    updating = true;
    progress = "Downloading...";
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          const total = event.data.contentLength;
          progress = total
            ? `Downloading (${(total / 1024 / 1024).toFixed(1)} MB)...`
            : "Downloading...";
        } else if (event.event === "Progress") {
          // keep showing download progress
        } else if (event.event === "Finished") {
          progress = "Restarting...";
        }
      });
      progress = "Restarting...";
      await relaunch();
    } catch (e) {
      console.error("Update failed:", e);
      progress = "Update failed";
      updating = false;
    }
  }
</script>

{#if update && !dismissed}
  <div
    class="mb-4 flex items-center justify-between rounded-lg bg-blue-50 dark:bg-blue-900/30 border border-blue-200 dark:border-blue-800 px-4 py-3"
  >
    <div class="flex items-center gap-2 text-sm">
      <svg
        xmlns="http://www.w3.org/2000/svg"
        class="h-4 w-4 text-blue-500"
        viewBox="0 0 20 20"
        fill="currentColor"
      >
        <path
          fill-rule="evenodd"
          d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-11a1 1 0 10-2 0v3.586L7.707 9.293a1 1 0 00-1.414 1.414l3 3a1 1 0 001.414 0l3-3a1 1 0 00-1.414-1.414L11 10.586V7z"
          clip-rule="evenodd"
        />
      </svg>
      {#if updating}
        <span class="text-blue-700 dark:text-blue-300">{progress}</span>
      {:else}
        <span class="text-blue-700 dark:text-blue-300">
          Conduit v{update.version} is available
        </span>
      {/if}
    </div>
    <div class="flex items-center gap-2">
      {#if !updating}
        <button
          class="px-3 py-1 text-xs font-medium rounded bg-blue-500 text-white hover:bg-blue-600"
          onclick={installUpdate}
        >
          Update Now
        </button>
        <button
          class="px-2 py-1 text-xs text-blue-500 hover:text-blue-700 dark:text-blue-400 dark:hover:text-blue-200"
          onclick={() => (dismissed = true)}
        >
          Later
        </button>
      {/if}
    </div>
  </div>
{/if}
