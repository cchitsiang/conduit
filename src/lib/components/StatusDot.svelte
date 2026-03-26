<script lang="ts">
  type Status = "connected" | "disconnected" | "connecting" | "error";

  let { status }: { status: Status } = $props();

  const colorMap: Record<Status, string> = {
    connected: "bg-green-500",
    disconnected: "bg-gray-400",
    connecting: "bg-yellow-400 animate-pulse",
    error: "bg-red-500",
  };
</script>

<span
  class="inline-block w-3 h-3 rounded-full {colorMap[status]}"
  title={status}
></span>
