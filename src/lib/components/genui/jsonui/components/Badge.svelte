<script lang="ts">
  import type { BaseComponentProps } from "@json-render/svelte";

  // A small status badge. `label` renders through Svelte text binding only
  // (never @html).
  interface BadgeProps {
    label: string;
    tone?: "info" | "success" | "warning" | "error";
  }

  let { props }: BaseComponentProps<BadgeProps> = $props();

  const toneClass = $derived(
    props.tone === "info"
      ? "text-info"
      : props.tone === "success"
        ? "text-success"
        : props.tone === "warning"
          ? "text-warning"
          : props.tone === "error"
            ? "text-error"
            : "text-base-content/70",
  );
</script>

<span class="inline-flex rounded px-2 py-0.5 text-xs break-words {toneClass}">
  {props.label}
</span>
