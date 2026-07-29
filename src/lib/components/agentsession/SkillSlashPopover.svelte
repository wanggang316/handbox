<script lang="ts">
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
  class="max-h-60 w-72 overflow-y-auto rounded-lg border border-[var(--hairline)] bg-base-200 py-1 shadow-lg"
  role="listbox"
  aria-label={t("agent.slash.ariaLabel")}
>
  {#if items.length === 0}
    <div class="px-3 py-2 text-xs text-base-content/50">
      {t("agent.slash.noMatch")}
    </div>
  {:else}
    {#each items as skill, index (skill.name)}
      {@const active = index === highlightedIndex}
      <button
        type="button"
        role="option"
        aria-selected={active}
        class={`flex w-full flex-col gap-0.5 px-3 py-1.5 text-left transition-colors ${
          active ? "bg-info/15 text-info" : "text-base-content/80 hover:bg-base-300"
        }`}
        onmousedown={(event) => {
          // mousedown (not click) so the textarea doesn't blur first and close the popover.
          event.preventDefault();
          onSelect(skill);
        }}
        onmouseenter={() => onHover(index)}
      >
        <span class="truncate text-sm font-medium">{skill.name}</span>
        {#if skill.description}
          <span class="truncate text-xs text-base-content/50">
            {skill.description}
          </span>
        {/if}
      </button>
    {/each}
  {/if}
</div>
