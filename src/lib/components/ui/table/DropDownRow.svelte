<script lang="ts">
  import DropDown from '../DropDown.svelte';
  import TableBaseRow from './TableBaseRow.svelte';

  interface DropDownOption {
    value: string;
    label: string;
    disabled?: boolean;
  }

  interface Props {
    label: string;
    options: DropDownOption[];
    selectedValue: string;
    description?: string;
    disabled?: boolean;
    onSelect?: (value: string, option: DropDownOption) => void;
  }

  let { 
    label,
    options = [],
    selectedValue = $bindable(),
    description = '',
    disabled = false,
    onSelect = (value: string, option: DropDownOption) => {}
  }: Props = $props();

  function handleSelect(value: string, option: DropDownOption) {
    selectedValue = value;
    onSelect(value, option);
  }
</script>

<TableBaseRow {label}>
  <div class="flex flex-col items-end">
    {#if description}
      <div class="text-xs text-gray-500 mb-1 text-right">
        {description}
      </div>
    {/if}
    <DropDown 
      {options}
      bind:selectedValue
      {disabled}
      onSelect={handleSelect}
    />
  </div>
</TableBaseRow>
