<script lang="ts">
  import type { Icon as IconType } from "@lucide/svelte";
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
     * Opens a detail view for this row. The whole row becomes the target — a
     * chevron beside the toggle asked the reader to hit a 16px glyph to reach
     * what the entire row is about.
     */
    onOpenDetail?: () => void;
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
  }: Props = $props();

  function handleToggleChange(value: boolean) {
    checked = value;
    onChange(value);
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    onOpenDetail?.();
  }
</script>

{#snippet iconSnippet(props: { class: string })}
  {#if icon}
    {@const RowIcon = icon}
    <RowIcon class={props.class} />
  {/if}
{/snippet}

{#snippet row()}
  <TableBaseRow
    {label}
    {description}
    {helpText}
    icon={icon ? iconSnippet : undefined}
  >
    <!-- The toggle swallows the click so flipping the switch never navigates. -->
    <div
      class="flex items-center"
      role="none"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <Toggle bind:checked {disabled} onChange={handleToggleChange} />
    </div>
  </TableBaseRow>
{/snippet}

{#if onOpenDetail}
  <!-- role="button" on a div, not a <button>: the row embeds the toggle's own
       control, and HTML forbids nesting one interactive element in another. -->
  <div
    role="button"
    tabindex="0"
    class="cursor-default transition-colors duration-[var(--dur-fast)] ease-[var(--ease-out)] hover:bg-base-300"
    onclick={onOpenDetail}
    onkeydown={handleKeydown}
  >
    {@render row()}
  </div>
{:else}
  {@render row()}
{/if}
