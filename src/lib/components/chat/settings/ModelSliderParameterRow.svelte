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
  }: Props = $props();
</script>

<TableBaseRow {label} layout="vertical">
  {#snippet rightContent()}
    <Toggle bind:checked={enabled} />
  {/snippet}

  {#if enabled}
    <LabeledSlider
      bind:value
      {min}
      {max}
      {step}
      {scaleMarks}
      {showScaleMarks}
      {showValue}
    />
  {/if}
</TableBaseRow>
