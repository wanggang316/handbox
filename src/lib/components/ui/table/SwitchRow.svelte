<script lang="ts">
  import type { Icon as IconType } from "@lucide/svelte";
  import { ChevronRight } from "@lucide/svelte";
  import Toggle from "../Toggle.svelte";
  import TableBaseRow from "./TableBaseRow.svelte";

  interface Props {
    label: string;
    checked: boolean;
    /** Optional glyph before the label (Lucide, or a component shaped like one). */
    icon?: typeof IconType;
    description?: string;
    helpText?: string;
    disabled?: boolean;
    onChange?: (value: boolean) => void;
    /**
     * Opens a detail view for this row. Rendered as its own chevron button
     * beside the toggle rather than by making the row clickable — the toggle is
     * interactive, and nesting it inside a row-wide button would be both
     * invalid markup and an ambiguous click target.
     */
    onOpenDetail?: () => void;
    /** Accessible name for the chevron; required when `onOpenDetail` is set. */
    detailAriaLabel?: string;
  }

  let {
    label,
    checked = $bindable(),
    icon = undefined,
    description = "",
    helpText = undefined,
    disabled = false,
    onChange = (_value: boolean) => {},
    onOpenDetail = undefined,
    detailAriaLabel = "",
  }: Props = $props();

  function handleToggleChange(value: boolean) {
    checked = value;
    onChange(value);
  }
</script>

{#snippet iconSnippet(props: { class: string })}
  {#if icon}
    {@const RowIcon = icon}
    <RowIcon class={props.class} />
  {/if}
{/snippet}

<TableBaseRow
  {label}
  {description}
  {helpText}
  icon={icon ? iconSnippet : undefined}
>
  <div class="flex items-center gap-1">
    <Toggle bind:checked {disabled} onChange={handleToggleChange} />
    {#if onOpenDetail}
      <button
        type="button"
        aria-label={detailAriaLabel}
        title={detailAriaLabel}
        class="flex size-7 items-center justify-center rounded-md text-base-content/40 transition-[color,background-color] duration-[var(--dur-fast)] ease-[var(--ease-out)] hover:bg-base-300 hover:text-base-content"
        onclick={onOpenDetail}
      >
        <ChevronRight size={16} />
      </button>
    {/if}
  </div>
</TableBaseRow>
