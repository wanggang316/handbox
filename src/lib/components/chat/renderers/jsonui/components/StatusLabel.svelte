<script lang="ts">
  import type { BaseComponentProps } from "@json-render/svelte";

  // A pill-shaped status label. `text` renders through Svelte text binding only
  // (never @html). The four states map to mutually distinguishable, semantically
  // clear tones: enabled→success (green), error→error (red), disabled→neutral
  // (grey), idle→info (secondary blue).
  interface StatusLabelProps {
    status: "enabled" | "disabled" | "idle" | "error";
    text: string;
  }

  let { props }: BaseComponentProps<StatusLabelProps> = $props();

  const statusClass = $derived(
    props.status === "enabled"
      ? "bg-success/10 text-success border-success/30"
      : props.status === "error"
        ? "bg-error/10 text-error border-error/30"
        : props.status === "idle"
          ? "bg-info/10 text-info border-info/30"
          : "bg-base-200 text-base-content/60 border-[var(--hairline)]",
  );
</script>

<span
  class="inline-flex items-center rounded-full border px-2.5 py-0.5 text-xs font-medium break-words {statusClass}"
>
  {props.text}
</span>
