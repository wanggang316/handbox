<script lang="ts">
  import type { TranslationData } from "./types";

  // All fields render through Svelte text binding `{value}` only — never `@html`.
  // This is the XSS safety boundary: any HTML/script markup in field values is
  // escaped to literal text by Svelte and can never execute or inject nodes.
  let {
    term,
    translation,
    phonetic,
    explanation,
  }: TranslationData = $props();
</script>

<div
  class="rounded-lg border border-[var(--hairline)] bg-base-300 p-4 text-sm text-base-content space-y-2"
>
  {#if term}
    <div class="text-base font-medium break-words">{term}</div>
  {/if}

  <div class="text-base-content break-words" class:text-base={!term}>
    {translation}
  </div>

  {#if phonetic}
    <div class="text-xs text-base-content/60 break-words">{phonetic}</div>
  {/if}

  {#if explanation}
    <div
      class="text-xs text-base-content/60 break-words whitespace-pre-wrap border-t border-[var(--hairline)] pt-2"
    >
      {explanation}
    </div>
  {/if}
</div>
