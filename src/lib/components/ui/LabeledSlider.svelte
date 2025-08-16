<script lang="ts">
  interface ScaleMark {
    value: number;
    position: number; // 在滑杆上的百分比位置
  }

  interface Props {
    label?: string;
    value: number;
    min?: number;
    max?: number;
    step?: number;
    leftLabel?: string;
    rightLabel?: string;
    scaleMarks?: ScaleMark[];
    description?: string;
    disabled?: boolean;
  }

  let { 
    label = '',
    value = $bindable(),
    min = 0,
    max = 100,
    step = 1,
    leftLabel = '',
    rightLabel = '',
    scaleMarks = [],
    description = '',
    disabled = false
  }: Props = $props();

  // 计算滑杆位置百分比
  let percentage = $derived(((value - min) / (max - min)) * 100);

  // 格式化显示值
  function formatValue(val: number): string {
    return val.toFixed(1);
  }
</script>

<div class="space-y-3">
  {#if label}
    <div class="flex items-center justify-between">
      <label for="labeled-slider-{label}" class="text-sm font-medium text-gray-700">{label}</label>
      <span class="text-sm font-mono text-gray-600 bg-gray-100 px-2 py-1 rounded">
        {formatValue(value)}
      </span>
    </div>
  {/if}

  <!-- 左右标签 -->
  {#if leftLabel || rightLabel}
    <div class="flex justify-between items-center text-xs text-gray-500 mb-2 px-1">
      <span>{leftLabel}</span>
      <span>{rightLabel}</span>
    </div>
  {/if}
  
  <div class="relative px-1">
    <!-- 滑杆轨道 -->
    <div class="relative h-1 bg-gray-200 rounded-full">
      <!-- 已填充部分 -->
      <div 
        class="absolute top-0 left-0 h-full bg-bg-accent rounded-full transition-all duration-200 ease-out"
        style="width: {percentage}%"
      ></div>
      
      <!-- 刻度圆环 -->
      {#each scaleMarks as mark}
        <div 
          class="absolute top-1/2 transform -translate-y-1/2 w-2 h-2 bg-white border border-gray-300 rounded-full" 
          style="left: calc({mark.position}% - 4px)"
        ></div>
      {/each}
      
      <!-- 滑块 -->
      <div 
        class="absolute top-1/2 transform -translate-y-1/2 w-4 h-4 bg-white border-2 border-bg-accent rounded-full shadow-md cursor-pointer transition-all duration-200 hover:scale-110 hover:shadow-lg focus:ring-4 focus:ring-blue-200 focus:outline-none z-10"
        style="left: calc({percentage}% - 8px)"
      ></div>
    </div>
    
    <!-- 隐藏的原生滑杆用于交互 -->
    <input
      id="labeled-slider-{label}"
      type="range"
      bind:value
      {min}
      {max}
      {step}
      {disabled}
      class="absolute top-0 left-0 w-full h-full opacity-0 cursor-pointer disabled:cursor-not-allowed z-20"
    />
  </div>

  <!-- 数值标签 -->
  {#if scaleMarks.length > 0}
    <div class="relative px-1 mt-2">
      <div class="relative h-4">
        {#each scaleMarks as mark}
          <span 
            class="absolute font-mono text-text-secondary text-xs transform -translate-x-1/2" 
            style="left: {mark.position}%"
          >
            {mark.value}
          </span>
        {/each}
      </div>
    </div>
  {/if}
  
  {#if description}
    <div class="text-xs text-gray-500">
      {description}
    </div>
  {/if}
</div>
