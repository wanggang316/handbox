<script lang="ts">
  import type { Snippet } from "svelte";

  // Shared page-level header so every page uses the same title typography.
  interface Props {
    title: string;
    /** Supplementary info next to the title (e.g. a count). */
    meta?: string;
    description?: string;
    /** Right-side action area (buttons etc.). */
    actions?: Snippet;
    /** Extension row below the title area (search box etc.). */
    children?: Snippet;
  }

  let { title, meta = "", description = "", actions, children }: Props =
    $props();
</script>

<header class="flex flex-col gap-3">
  <div class="flex items-center justify-between gap-4">
    <div class="flex min-w-0 items-baseline gap-2.5">
      <h1 class="truncate text-2xl font-semibold text-base-content">{title}</h1>
      {#if meta}
        <span class="shrink-0 text-sm text-base-content/50">{meta}</span>
      {/if}
    </div>
    {#if actions}
      <div class="flex shrink-0 items-center gap-2">{@render actions()}</div>
    {/if}
  </div>
  {#if description}
    <p class="text-sm text-base-content/60">{description}</p>
  {/if}
  {#if children}
    {@render children()}
  {/if}
</header>
