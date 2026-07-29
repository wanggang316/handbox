<script lang="ts">
  // Shared loading spinner: neutral gray ring with a darker rotating arc.
  interface Props {
    size?: number;
    class?: string;
  }
  let { size = 24, class: className = "" }: Props = $props();
</script>

<div
  class="animate-spin rounded-full border-2 border-base-300 border-t-base-content/50 {className}"
  style="width: {size}px; height: {size}px;"
  role="status"
  aria-label="loading"
></div>
