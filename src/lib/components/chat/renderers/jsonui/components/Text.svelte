<script lang="ts">
  import type { BaseComponentProps } from "@json-render/svelte";

  // A text block. `text` renders through Svelte text binding only (never @html),
  // so any HTML/script markup is escaped to literal characters.
  interface TextProps {
    text: string;
    variant?: "body" | "heading" | "muted";
  }

  let { props }: BaseComponentProps<TextProps> = $props();

  const variantClass = $derived(
    props.variant === "heading"
      ? "text-base font-medium text-base-content"
      : props.variant === "muted"
        ? "text-base-content/60 text-sm"
        : "text-base-content text-sm",
  );
</script>

<div class="break-words {variantClass}">{props.text}</div>
