<script lang="ts">
  import Select from '../Select.svelte';
  import TableBaseRow from './TableBaseRow.svelte';

  interface Option {
    value: string;
    label: string;
  }

  interface Props {
    label: string;
    options?: Option[];
    selectedValue: string;
    description?: string;
    helpText?: string;
    disabled?: boolean;
    onSelect?: (value: string) => void;
  }

  let {
    label,
    options = [],
    selectedValue = $bindable(),
    description = '',
    helpText = undefined,
    disabled = false,
    onSelect = (value: string) => {}
  }: Props = $props();

  function handleSelect(value: string) {
    selectedValue = value;
    onSelect(value);
  }
</script>

<TableBaseRow {label} {helpText} py=2>
  <div class="flex flex-col items-end">
    {#if description}
      <div class="text-xs text-base-content/70 mb-1 text-right">
        {description}
      </div>
    {/if}
    <Select
      {options}
      bind:selectedValue
      {disabled}
      onChange={handleSelect}
      autoWidth={true}
      size="sm"
    />
  </div>
</TableBaseRow>
