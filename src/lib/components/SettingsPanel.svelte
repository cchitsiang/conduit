<script lang="ts">
  import { settings, saveSettings, providers } from "$lib/stores/vpn";
  import type { AppSettings } from "$lib/types";

  let { open, onclose }: { open: boolean; onclose: () => void } = $props();

  let localSettings = $state<AppSettings>({ ...$settings });

  $effect(() => {
    if (open) {
      localSettings = { ...$settings };
    }
  });

  async function handleSave() {
    await saveSettings(localSettings);
    onclose();
  }

  function updateVisibility(name: string, visible: boolean) {
    localSettings.provider_visibility = {
      ...localSettings.provider_visibility,
      [name]: visible,
    };
  }
</script>

{#if open}
  <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
    <div class="bg-white dark:bg-gray-800 rounded-xl p-6 w-96 shadow-xl">
      <h2 class="text-lg font-semibold text-gray-800 dark:text-gray-100 mb-4">
        Settings
      </h2>

      <div class="space-y-4">
        <div>
          <label for="poll-interval" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            Poll Interval (seconds)
          </label>
          <input
            id="poll-interval"
            type="range"
            min="1"
            max="10"
            bind:value={localSettings.poll_interval_secs}
            class="w-full"
          />
          <span class="text-sm text-gray-500">{localSettings.poll_interval_secs}s</span>
        </div>

        <div>
          <label class="flex items-center justify-between text-sm">
            <span class="text-gray-700 dark:text-gray-300">Launch at Login</span>
            <input
              type="checkbox"
              bind:checked={localSettings.launch_at_login}
              class="rounded"
            />
          </label>
        </div>

        <div>
          <p class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
            Provider Visibility
          </p>
          {#each $providers as provider}
            <label class="flex items-center justify-between text-sm py-1">
              <span class="text-gray-600 dark:text-gray-400">{provider.name}</span>
              <input
                type="checkbox"
                checked={localSettings.provider_visibility[provider.name] !== false}
                onchange={(e) =>
                  updateVisibility(
                    provider.name,
                    (e.target as HTMLInputElement).checked,
                  )}
                class="rounded"
              />
            </label>
          {/each}
        </div>
      </div>

      <div class="flex justify-end gap-2 mt-6">
        <button
          class="px-4 py-2 text-sm text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200"
          onclick={onclose}
        >
          Cancel
        </button>
        <button
          class="px-4 py-2 text-sm bg-blue-500 text-white rounded-lg hover:bg-blue-600"
          onclick={handleSave}
        >
          Save
        </button>
      </div>
    </div>
  </div>
{/if}
