<script lang="ts">
  import type { BaseComponentProps } from "@json-render/svelte";

  // A bordered card container. `title` (when present) renders text-bound (never
  // @html) above the children slot — the XSS-safe boundary for AI-generated specs.
  interface CardProps {
    title?: string;
  }

  let { props, children }: BaseComponentProps<CardProps> = $props();
</script>

<div class="rounded-lg border border-[var(--hairline)] bg-base-300 p-3">
  {#if props.title}
    <div class="font-medium text-base-content break-words">{props.title}</div>
  {/if}
  {@render children?.()}
</div>
