<script lang="ts">
  import { Zap } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  import type { SkillInfo } from "$lib/types";

  interface Props {
    /** Pre-filtered candidates (enabled skills matching the query). */
    items: SkillInfo[];
    /** Highlighted index; [0, items.length) when non-empty, -1 when empty. */
    highlightedIndex: number;
    /** Click callback (same as selecting with Enter). */
    onSelect: (skill: SkillInfo) => void;
    /** Syncs the highlight on hover so keyboard and mouse stay unified. */
    onHover: (index: number) => void;
  }

  let { items, highlightedIndex, onSelect, onHover }: Props = $props();

  // Scroll the highlighted item into view so keyboard navigation follows past
  // the max-h boundary; `nearest` only scrolls when the item is not visible.
  let listRef = $state<HTMLDivElement>();
  $effect(() => {
    const idx = highlightedIndex;
    if (idx < 0 || !listRef) return;
    const active = listRef.querySelector('[aria-selected="true"]');
    if (active instanceof HTMLElement) {
      active.scrollIntoView({ block: "nearest" });
    }
  });
</script>

<!--
  Skill autocomplete popover anchored to the textarea. The parent positions it
  (absolute, bottom-full) and routes all keyboard behavior from the textarea's
  keydown; this component only renders the list and highlight.
  a11y: listbox/option roles + aria-selected make the highlight readable.
-->
<div
  bind:this={listRef}
  class="max-h-72 w-56 overflow-y-auto rounded-lg border border-[var(--hairline)] bg-base-100 p-1 shadow-lg"
  role="listbox"
  aria-label={t("agent.slash.ariaLabel")}
>
  {#if items.length === 0}
    <div class="px-2 py-1.5 text-xs text-base-content/50">
      {t("agent.slash.noMatch")}
    </div>
  {:else}
    {#each items as skill, index (skill.name)}
      {@const active = index === highlightedIndex}
      <button
        type="button"
        role="option"
        aria-selected={active}
        class={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-base-300 ${
          active ? "bg-base-300" : ""
        }`}
        onmousedown={(event) => {
          // mousedown (not click) so the textarea doesn't blur first and close the popover.
          event.preventDefault();
          onSelect(skill);
        }}
        onmouseenter={() => onHover(index)}
      >
        <Zap size={16} class="shrink-0 text-base-content/70" />
        <span class="min-w-0 flex-1 truncate text-sm text-base-content">
          {skill.name}
        </span>
      </button>
    {/each}
  {/if}
</div>
