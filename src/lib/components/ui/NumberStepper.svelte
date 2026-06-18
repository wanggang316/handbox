<script lang="ts">
  import { ChevronUp, ChevronDown } from '@lucide/svelte';
  import { t } from '$lib/i18n';

  interface Props {
    value: number;
    min?: number;
    max?: number;
    step?: number;
    formatValue?: (value: number) => string;
    placeholder?: string;
    defaultValue?: number;
    disabled?: boolean;
  }

  let { 
    value = $bindable(),
    min = 0,
    max = Infinity,
    step = 1,
    formatValue = (val: number) => val.toString(),
    placeholder = '',
    defaultValue,
    disabled = false
  }: Props = $props();

  // 当当前值等于默认值时，显示 placeholder 而不是实际值
  const shouldShowPlaceholder = $derived(defaultValue !== undefined && value === defaultValue);
  const displayValue = $derived(shouldShowPlaceholder ? '' : value);

  function increment() {
    if (disabled || value + step > max) return;
    value = Math.min(value + step, max);
  }

  function decrement() {
    if (disabled || value - step < min) return;
    value = Math.max(value - step, min);
  }

  function handleInput(event: Event) {
    const target = event.target as HTMLInputElement;
    const newValue = parseInt(target.value);
    if (!isNaN(newValue)) {
      value = Math.max(min, Math.min(max, newValue));
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      increment();
    } else if (event.key === 'ArrowDown') {
      event.preventDefault();
      decrement();
    }
  }
</script>

<div class="relative flex items-center">
  <input
    type="number"
    value={displayValue}
    {min}
    {max}
    {step}
    {placeholder}
    {disabled}
    oninput={handleInput}
    onkeydown={handleKeydown}
    class="flex-1 px-3 py-1 pr-7 text-sm text-right bg-transparent disabled:opacity-50 disabled:cursor-not-allowed"
  />
  
  <div class="absolute right-1 flex flex-col">
    <button
      type="button"
      onclick={increment}
      disabled={disabled || value >= max}
      class="p-0.5 text-base-content/60 hover:text-base-content disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
      aria-label={t('ui.increase')}
    >
      <ChevronUp size={12} />
    </button>
    <button
      type="button"
      onclick={decrement}
      disabled={disabled || value <= min}
      class="p-0.5 text-base-content/60 hover:text-base-content disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
      aria-label={t('ui.decrease')}
    >
      <ChevronDown size={12} />
    </button>
  </div>
</div>

<style>
  /* 隐藏默认的数字输入框步进按钮 */
  input[type="number"]::-webkit-outer-spin-button,
  input[type="number"]::-webkit-inner-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }
  
  input[type="number"] {
    appearance: textfield;
    -moz-appearance: textfield;
  }
</style>
