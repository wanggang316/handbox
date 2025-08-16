<script lang="ts">
  import NumberStepper from './NumberStepper.svelte';

  interface Props {
    label: string;
    value: number;
    min?: number;
    max?: number;
    step?: number;
    placeholder?: string;
    disabled?: boolean;
  }

  let { 
    label,
    value = $bindable(),
    min = 0,
    max = Infinity,
    step = 1,
    placeholder = '',
    disabled = false
  }: Props = $props();
</script>

<div class="flex items-center justify-between w-full">
  <div class="text-sm font-medium text-gray-700">{label}</div>
  <div class="flex-1 max-w-24 ml-4">
    <NumberStepper 
      bind:value
      {min}
      {max}
      {step}
      {placeholder}
      {disabled}
    />
  </div>
</div>
