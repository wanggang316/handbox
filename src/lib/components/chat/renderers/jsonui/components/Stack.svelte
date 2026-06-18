<script lang="ts">
  import type { BaseComponentProps } from "@json-render/svelte";

  // A flex layout container. Items are supplied through the children slot.
  interface StackProps {
    gap?: "sm" | "md" | "lg";
    direction?: "row" | "col";
  }

  let { props, children }: BaseComponentProps<StackProps> = $props();

  const directionClass = $derived(
    props.direction === "row" ? "flex flex-row" : "flex flex-col",
  );
  const gapClass = $derived(
    props.gap === "sm" ? "gap-1" : props.gap === "lg" ? "gap-4" : "gap-2",
  );
</script>

<div class="{directionClass} {gapClass}">
  {@render children?.()}
</div>
