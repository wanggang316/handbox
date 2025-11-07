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
    enabled?: boolean;
    onToggleChange?: (enabled: boolean) => void;
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
    enabled = $bindable(true),
    onToggleChange,
  }: Props = $props();
</script>

<TableBaseRow {label} layout="vertical">
  {#snippet rightContent()}
    <Toggle bind:checked={enabled} onChange={onToggleChange} />
  {/snippet}

  {#if enabled}
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
