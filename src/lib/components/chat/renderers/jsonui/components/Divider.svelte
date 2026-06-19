<script lang="ts">
  import type { BaseComponentProps } from "@json-render/svelte";

  // A thin separator line drawn with `var(--hairline)`. `orientation` selects a
  // full-width horizontal rule (default) or a full-height vertical rule.
  interface DividerProps {
    orientation?: "horizontal" | "vertical";
  }

  let { props }: BaseComponentProps<DividerProps> = $props();

  const orientationClass = $derived(
    props.orientation === "vertical"
      ? "self-stretch w-px"
      : "w-full h-px",
  );
</script>

<div class="bg-[var(--hairline)] {orientationClass}" role="separator"></div>
