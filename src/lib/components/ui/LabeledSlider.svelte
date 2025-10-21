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

  // 计算滑杆位置百分比
  let percentage = $derived(((value - min) / (max - min)) * 100);

  // 格式化显示值
  function formatValue(val: number): string {
    return val.toFixed(1);
  }
</script>

<div class="space-y-1">
  <!-- 左右标签 -->
  {#if leftLabel || rightLabel}
    <div class="flex items-center gap-3">
      <div
        class="flex-1 flex justify-between items-center text-xs text-base-content/70 px-1"
      >
        <span>{leftLabel}</span>
        <span>{rightLabel}</span>
      </div>
      {#if showValue}
        <div class="min-w-[3rem]"></div>
      {/if}
    </div>
  {/if}

  <div class="flex items-center gap-3">
    <div class="relative flex-1 px-1">
      <!-- 滑杆轨道 -->
      <div class="relative h-1 bg-base-300 rounded-full">
        <!-- 已填充部分 -->
        <div
          class="absolute top-0 left-0 h-full bg-primary rounded-full transition-all duration-200 ease-out"
          style="width: {percentage}%"
        ></div>

        <!-- 刻度圆环 -->
        {#if showScaleMarks}
          {#each scaleMarks as mark}
            <div
              class="absolute top-1/2 transform -translate-y-1/2 w-2 h-2 bg-base-100 border border-base-300 rounded-full"
              style="left: calc({mark.position}% - 4px)"
            ></div>
          {/each}
        {/if}

        <!-- 滑块 -->
        <div
          class="absolute top-1/2 transform -translate-y-1/2 w-4 h-4 bg-base-100 border-2 border-primary rounded-full shadow-md cursor-pointer transition-all duration-200 hover:scale-110 hover:shadow-lg focus:ring-4 focus:ring-primary/30 focus:outline-none z-10"
          style="left: calc({percentage}% - 8px)"
        ></div>
      </div>

      <!-- 隐藏的原生滑杆用于交互 -->
      <input
        type="range"
        bind:value
        {min}
        {max}
        {step}
        {disabled}
        class="absolute top-0 left-0 w-full h-full opacity-0 cursor-pointer disabled:cursor-not-allowed z-20"
      />
    </div>

    <!-- 数值显示 - 在滑杆右侧 -->
    {#if showValue}
      <span
        class="text-sm font-mono text-base-content/80 bg-base-300 px-2 py-1 rounded-lg whitespace-nowrap min-w-[3rem] text-center"
      >
        {formatValue(value)}
      </span>
    {/if}
  </div>

  <!-- 数值标签 -->
  {#if showScaleMarks && scaleMarks.length > 0}
    <div class="flex items-center gap-3">
      <div class="relative flex-1 px-1">
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
        <div class="min-w-[3rem]"></div>
      {/if}
    </div>
  {/if}

  {#if description}
    <div class="text-xs text-base-content/70">
      {description}
    </div>
  {/if}
</div>
