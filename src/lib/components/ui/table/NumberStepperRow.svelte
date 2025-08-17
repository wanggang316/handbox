<script lang="ts">
  import NumberStepper from '../NumberStepper.svelte';
  import TableBaseRow from './TableBaseRow.svelte';

  interface Props {
    label: string;
    value: number;
    min?: number;
    max?: number;
    step?: number;
    defaultValue?: number;
    placeholder?: string;
    disabled?: boolean;
  }

  let { 
    label,
    value = $bindable(),
    min = 0,
    max = Infinity,
    step = 1,
    defaultValue,
    placeholder = '',
    disabled = false
  }: Props = $props();


</script>

<TableBaseRow {label}>
  <div class="flex-1 max-w-100">
    <NumberStepper 
      bind:value
      {min}
      {max}
      {step}
      {defaultValue}
      {placeholder}
      {disabled}
    />
  </div>
</TableBaseRow>
