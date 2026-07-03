<script lang="ts">
  // 统一的 loading spinner：中性灰圈 + 深色转角，尺寸经 size 控制。
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
