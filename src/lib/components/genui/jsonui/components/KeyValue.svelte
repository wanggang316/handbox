<script lang="ts">
  import type { BaseComponentProps } from "@json-render/svelte";

  // A list of key/value rows. One row per `items` entry, rendered as
  // "key — value". Both `key` and `value` render through Svelte text binding only
  // (never @html). An empty `items` array is valid and renders nothing.
  interface KeyValueProps {
    items: { key: string; value: string }[];
  }

  let { props }: BaseComponentProps<KeyValueProps> = $props();
</script>

<div class="flex flex-col gap-1">
  {#each props.items as item, i (i)}
    <div class="flex gap-2 text-sm break-words">
      <span class="font-medium text-base-content/70">{item.key}</span>
      <span class="text-base-content/50">—</span>
      <span class="text-base-content">{item.value}</span>
    </div>
  {/each}
</div>
