<script lang="ts">
  import Toggle from "../Toggle.svelte";
  import TableBaseRow from "./TableBaseRow.svelte";

  interface Props {
    label: string;
    checked: boolean;
    description?: string;
    helpText?: string;
    disabled?: boolean;
    onChange?: (value: boolean) => void;
  }

  let {
    label,
    checked = $bindable(),
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

<TableBaseRow {label} {helpText}>
  <div class="flex flex-col items-end">
    {#if description}
      <div class="text-xs text-base-content/70 mb-1 text-right">
        {description}
      </div>
    {/if}
    <Toggle bind:checked {disabled} onChange={handleToggleChange} />
  </div>
</TableBaseRow>
