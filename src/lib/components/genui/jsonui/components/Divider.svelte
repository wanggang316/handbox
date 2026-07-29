<script lang="ts">
  import type { BaseComponentProps } from "@json-render/svelte";

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
