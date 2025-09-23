<script lang="ts">
  import LabeledSlider from '../LabeledSlider.svelte';
  import TableBaseRow from './TableBaseRow.svelte';

  interface ScaleMark {
    value: number;
    position: number;
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

  // 格式化显示值
  function formatValue(val: number): string {
    return val.toFixed(1);
  }
</script>

<TableBaseRow {label} layout="vertical">
  {#snippet rightContent()}
    <span class="text-sm font-mono text-base-content/80 bg-base-200 px-2 py-1 rounded">
      {formatValue(value)}
    </span>
  {/snippet}
  
  <LabeledSlider 
    bind:value
    {min}
    {max}
    {step}
    {leftLabel}
    {rightLabel}
    {scaleMarks}
    {description}
    {disabled}
    showValue={false}
  />
</TableBaseRow>
