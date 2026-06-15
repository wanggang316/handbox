<script lang="ts">
  interface Props {
    label?: string;
    value: number;
    min?: number;
    max?: number;
    step?: number;
    formatValue?: (value: number) => string;
    description?: string;
    disabled?: boolean;
  }

  let { 
    label = '',
    value = $bindable(),
    min = 0,
    max = 100,
    step = 1,
    formatValue = (val: number) => val.toString(),
    description = '',
    disabled = false
  }: Props = $props();

  // 计算滑杆位置百分比
  let percentage = $derived(((value - min) / (max - min)) * 100);
</script>

<div class="space-y-2">
  {#if label}
    <div class="flex items-center justify-between">
      <label for="slider-{label}" class="text-sm font-medium text-base-content">{label}</label>
      <span class="text-sm font-mono text-base-content/80 bg-base-300 px-2 py-0.5 rounded-md">
        {formatValue(value)}
      </span>
    </div>
  {/if}
  
  <div class="relative">
    <input
      id="slider-{label}"
      type="range"
      bind:value
      {min}
      {max}
      {step}
      {disabled}
      class="slider w-full h-2 bg-base-300 rounded-lg appearance-none cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
      style="background: linear-gradient(to right, var(--primary) 0%, var(--primary) {percentage}%, var(--base-300) {percentage}%, var(--base-300) 100%)"
    />
  </div>
  
  {#if description}
    <div class="text-xs text-base-content/70">
      {description}
    </div>
  {/if}
</div>

<style>
  .slider::-webkit-slider-thumb {
    appearance: none;
    height: 20px;
    width: 20px;
    border-radius: 50%;
    background: var(--primary);
    cursor: pointer;
    border: 2px solid var(--base-100);
    box-shadow: 0 2px 4px color-mix(in oklch, var(--base-content) 15%, transparent);
    transition: all 0.2s ease;
  }

  .slider::-webkit-slider-thumb:hover {
    filter: brightness(1.1);
    transform: scale(1.1);
    box-shadow: 0 4px 8px color-mix(in oklch, var(--base-content) 20%, transparent);
  }

  .slider::-moz-range-thumb {
    height: 20px;
    width: 20px;
    border-radius: 50%;
    background: var(--primary);
    cursor: pointer;
    border: 2px solid var(--base-100);
    box-shadow: 0 2px 4px color-mix(in oklch, var(--base-content) 15%, transparent);
    transition: all 0.2s ease;
  }

  .slider::-moz-range-thumb:hover {
    filter: brightness(1.1);
    transform: scale(1.1);
    box-shadow: 0 4px 8px color-mix(in oklch, var(--base-content) 20%, transparent);
  }

  .slider:disabled::-webkit-slider-thumb {
    background: color-mix(in oklch, var(--base-content) 40%, transparent);
    cursor: not-allowed;
    transform: none;
  }

  .slider:disabled::-moz-range-thumb {
    background: color-mix(in oklch, var(--base-content) 40%, transparent);
    cursor: not-allowed;
    transform: none;
  }
</style>
