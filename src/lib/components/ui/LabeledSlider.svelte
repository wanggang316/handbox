<script lang="ts">
  interface ScaleMark {
    value: number;
    position: number; // percentage position along the track
  }

  interface Props {
    value: number;
    min?: number;
    max?: number;
    step?: number;
    leftLabel?: string;
    rightLabel?: string;
    scaleMarks?: ScaleMark[];
    description?: string;
    showValue?: boolean;
    showScaleMarks?: boolean;
    disabled?: boolean;
  }

  let {
    value = $bindable(),
    min = 0,
    max = 100,
    step = 1,
    leftLabel = "",
    rightLabel = "",
    scaleMarks = [],
    description = "",
    showValue = true,
    showScaleMarks = true,
    disabled = false,
  }: Props = $props();

  function getDecimalPlaces(stepValue: number): number {
    if (stepValue >= 1) return 0;
    const stepStr = stepValue.toString();
    const decimalIndex = stepStr.indexOf(".");
    if (decimalIndex === -1) return 0;
    return stepStr.length - decimalIndex - 1;
  }

  const decimalPlaces = $derived(getDecimalPlaces(step));

  // Draft text of the number box while it is being edited.
  let inputValue = $state("");
  let isEditing = $state(false);

  // Sync from value only while not editing, so typing isn't clobbered.
  $effect(() => {
    if (!isEditing) {
      inputValue = value.toFixed(decimalPlaces);
    }
  });

  function handleInput(event: Event) {
    const target = event.target as HTMLInputElement;
    inputValue = target.value;
  }

  function handleBlur() {
    isEditing = false;
    const parsed = parseFloat(inputValue);

    if (isNaN(parsed)) {
      inputValue = value.toFixed(decimalPlaces);
      return;
    }

    let clamped = Math.max(min, Math.min(max, parsed));

    if (step > 0) {
      clamped = Math.round((clamped - min) / step) * step + min;
      // Trim float precision artifacts from the snap.
      clamped = parseFloat(clamped.toFixed(10));
    }

    value = clamped;
    inputValue = clamped.toFixed(decimalPlaces);
  }

  function handleFocus() {
    isEditing = true;
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      (event.target as HTMLInputElement).blur();
    } else if (event.key === "Escape") {
      // Escape cancels the edit.
      inputValue = value.toFixed(decimalPlaces);
      (event.target as HTMLInputElement).blur();
    }
  }
</script>

<div class="space-y-1">
  {#if leftLabel || rightLabel}
    <div class="flex items-center gap-3">
      <div
        class="flex-1 flex justify-between items-center text-xs text-base-content/70"
      >
        <span>{leftLabel}</span>
        <span>{rightLabel}</span>
      </div>
      {#if showValue}
        <div class="w-auto"></div>
      {/if}
    </div>
  {/if}

  <div class="flex items-center gap-3">
    <input
      type="range"
      bind:value
      {min}
      {max}
      {step}
      {disabled}
      class="native-slider flex-1"
    />

    {#if showValue}
      <input
        type="text"
        value={inputValue}
        oninput={handleInput}
        onfocus={handleFocus}
        onblur={handleBlur}
        onkeydown={handleKeydown}
        {disabled}
        size={inputValue.length || 4}
        class="text-sm font-mono text-base-content/80 bg-base-300 px-2 py-1 rounded-lg whitespace-nowrap w-auto text-center border border-transparent hover:border-base-content/20 focus:border-base-content/20 disabled:opacity-50 disabled:cursor-not-allowed"
      />
    {/if}
  </div>

  {#if showScaleMarks && scaleMarks.length > 0}
    <div class="flex items-center gap-3">
      <div class="relative flex-1">
        <div class="relative h-4">
          {#each scaleMarks as mark}
            <span
              class="absolute font-mono text-base-content/80 text-xs transform -translate-x-1/2"
              style="left: {mark.position}%"
            >
              {mark.value}
            </span>
          {/each}
        </div>
      </div>
      {#if showValue}
        <div class="w-auto"></div>
      {/if}
    </div>
  {/if}

  {#if description}
    <div class="text-xs text-base-content/70">
      {description}
    </div>
  {/if}
</div>

<style>
  .native-slider {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 4px;
    background: var(--color-base-300);
    border-radius: 9999px;
    outline: none;
    cursor: pointer;
  }

  .native-slider:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .native-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 16px;
    height: 16px;
    background: var(--color-base-100);
    border: 2px solid var(--color-primary);
    border-radius: 50%;
    cursor: pointer;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  }

  .native-slider::-webkit-slider-thumb:hover {
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.15);
  }

  .native-slider::-moz-range-thumb {
    width: 16px;
    height: 16px;
    background: var(--color-base-100);
    border: 2px solid var(--color-primary);
    border-radius: 50%;
    cursor: pointer;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  }

  .native-slider::-moz-range-thumb:hover {
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.15);
  }

  .native-slider::-moz-range-progress {
    background: var(--color-primary);
    border-radius: 9999px;
    height: 4px;
  }
</style>
