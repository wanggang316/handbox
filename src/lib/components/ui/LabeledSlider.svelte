<script lang="ts">
  interface ScaleMark {
    value: number;
    position: number; // 在滑杆上的百分比位置
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

  // 根据 step 自动计算小数位数
  function getDecimalPlaces(stepValue: number): number {
    if (stepValue >= 1) return 0; // step >= 1，显示整数
    const stepStr = stepValue.toString();
    const decimalIndex = stepStr.indexOf(".");
    if (decimalIndex === -1) return 0;
    // 计算小数点后的位数
    return stepStr.length - decimalIndex - 1;
  }

  const decimalPlaces = $derived(getDecimalPlaces(step));

  // 输入框内部值（字符串，用于编辑）
  let inputValue = $state("");
  let isEditing = $state(false);

  // 当外部 value 或 decimalPlaces 变化时，更新 inputValue（仅在非编辑状态）
  $effect(() => {
    if (!isEditing) {
      inputValue = value.toFixed(decimalPlaces);
    }
  });

  // 处理输入框的输入事件
  function handleInput(event: Event) {
    const target = event.target as HTMLInputElement;
    inputValue = target.value;
  }

  // 处理输入框失焦 - 验证并更新 value
  function handleBlur() {
    isEditing = false;
    const parsed = parseFloat(inputValue);

    if (isNaN(parsed)) {
      // 无效输入，恢复到当前值
      inputValue = value.toFixed(decimalPlaces);
      return;
    }

    // 限制在 min-max 范围内，并按 step 对齐
    let clamped = Math.max(min, Math.min(max, parsed));

    // 对齐到 step
    if (step > 0) {
      clamped = Math.round((clamped - min) / step) * step + min;
      // 修正浮点数精度问题
      clamped = parseFloat(clamped.toFixed(10));
    }

    value = clamped;
    inputValue = clamped.toFixed(decimalPlaces);
  }

  // 处理输入框获取焦点
  function handleFocus() {
    isEditing = true;
  }

  // 处理回车键 - 提交并失焦
  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      (event.target as HTMLInputElement).blur();
    } else if (event.key === "Escape") {
      // ESC 键取消编辑
      inputValue = value.toFixed(decimalPlaces);
      (event.target as HTMLInputElement).blur();
    }
  }
</script>

<div class="space-y-1">
  <!-- 左右标签 -->
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
    <!-- 原生滑杆 (使用 CSS 样式美化) -->
    <input
      type="range"
      bind:value
      {min}
      {max}
      {step}
      {disabled}
      class="native-slider flex-1"
    />

    <!-- 数值输入框 - 在滑杆右侧 -->
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
        class="text-sm font-mono text-base-content/80 bg-base-300 px-2 py-1 rounded-lg whitespace-nowrap w-auto text-center border border-transparent hover:border-base-content/20 focus:border-primary focus:outline-none disabled:opacity-50 disabled:cursor-not-allowed"
      />
    {/if}
  </div>

  <!-- 数值标签 (仅在需要时显示) -->
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
  /* 原生滑杆样式优化 - 高性能版本 */
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

  /* WebKit 浏览器 (Chrome, Safari, Edge) - 滑块 */
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

  /* Firefox - 滑块 */
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

  /* Firefox - 进度条 */
  .native-slider::-moz-range-progress {
    background: var(--color-primary);
    border-radius: 9999px;
    height: 4px;
  }
</style>
