<script lang="ts">
  import Toggle from '../Toggle.svelte';
  import TableBaseRow from './TableBaseRow.svelte';

  interface Props {
    label: string;
    checked: boolean;
    description?: string;
    disabled?: boolean;
  }

  let { 
    label,
    checked = $bindable(),
    description = '',
    disabled = false
  }: Props = $props();
</script>

<TableBaseRow {label}>
  <div class="flex flex-col items-end">
    {#if description}
      <div class="text-xs text-gray-500 mb-1 text-right">
        {description}
      </div>
    {/if}
    <Toggle bind:checked {disabled} />
  </div>
</TableBaseRow>
