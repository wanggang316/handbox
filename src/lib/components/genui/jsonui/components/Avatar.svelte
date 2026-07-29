<script lang="ts">
  import type { BaseComponentProps } from "@json-render/svelte";

  // Display-only: no image src, network proxy, or upload path, so an
  // AI-generated spec cannot trigger a fetch or upload. `letter` renders through
  // Svelte text binding only (never @html).
  interface AvatarProps {
    letter: string;
    size?: "sm" | "md" | "lg";
  }

  let { props }: BaseComponentProps<AvatarProps> = $props();

  const sizeClass = $derived(
    props.size === "sm"
      ? "w-8 h-8 text-xs"
      : props.size === "lg"
        ? "w-16 h-16 text-xl"
        : "w-12 h-12 text-base",
  );

  const initial = $derived(props.letter.charAt(0).toUpperCase());
</script>

<div
  class="flex shrink-0 items-center justify-center rounded-full bg-base-300 font-semibold text-base-content/80 break-words {sizeClass}"
>
  {initial}
</div>
