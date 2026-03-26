<script lang="ts">
  let {
    checked,
    disabled = false,
    onchange,
  }: {
    checked: boolean;
    disabled?: boolean;
    onchange: (checked: boolean) => void;
  } = $props();
</script>

<button
  role="switch"
  aria-checked={checked}
  aria-label="Toggle"
  {disabled}
  class="relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 {checked
    ? 'bg-green-500'
    : 'bg-gray-300'} {disabled ? 'opacity-50 cursor-not-allowed' : ''}"
  onclick={() => !disabled && onchange(!checked)}
>
  <span
    class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out {checked
      ? 'translate-x-5'
      : 'translate-x-0'}"
  ></span>
</button>
