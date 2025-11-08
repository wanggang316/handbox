<script lang="ts">
  import Toggle from "../../ui/Toggle.svelte";
  import LabeledSlider from "../../ui/LabeledSlider.svelte";
  import TableBaseRow from "../../ui/table/TableBaseRow.svelte";

  interface Props {
    label: string;
    value?: number;
    min: number;
    max: number;
    step: number;
    scaleMarks?: Array<{ value: number; position: number }>;
    showScaleMarks?: boolean;
    showValue?: boolean;
    showToggle?: boolean; // 是否显示开关，默认显示
    enabled?: boolean;
    onToggleChange?: (enabled: boolean) => void;
    helpText?: string; // 可选的帮助提示文本
  }

  let {
    label,
    value = $bindable(0),
    min,
    max,
    step,
    scaleMarks = [],
    showScaleMarks = false,
    showValue = true,
    showToggle = true,
    enabled = $bindable(true),
    onToggleChange,
    helpText,
  }: Props = $props();

  // 如果不显示开关，总是启用
  const isEnabled = $derived(showToggle ? enabled : true);
</script>

{#if showToggle}
  <TableBaseRow {label} layout="vertical" {helpText}>
    {#snippet rightContent()}
      <Toggle bind:checked={enabled} onChange={onToggleChange} />
    {/snippet}

    {#if isEnabled}
      <div class="pt-2">
        <LabeledSlider
          bind:value
          {min}
          {max}
          {step}
          {scaleMarks}
          {showScaleMarks}
          {showValue}
        />
      </div>
    {/if}
  </TableBaseRow>
{:else}
  <TableBaseRow {label} layout="vertical" {helpText}>
    {#if isEnabled}
      <div class="pt-2">
        <LabeledSlider
          bind:value
          {min}
          {max}
          {step}
          {scaleMarks}
          {showScaleMarks}
          {showValue}
        />
      </div>
    {/if}
  </TableBaseRow>
{/if}
