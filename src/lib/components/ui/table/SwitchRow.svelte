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
  }

  let {
    label,
    checked = $bindable(),
    icon = undefined,
    description = "",
    helpText = undefined,
    disabled = false,
    onChange = (_value: boolean) => {},
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
  <Toggle bind:checked {disabled} onChange={handleToggleChange} />
</TableBaseRow>
