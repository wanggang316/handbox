<script lang="ts">
  import { RadioGroup } from "bits-ui";
  import { getFormField } from "./FormField.svelte";

  interface Option {
    value: string;
    label: string;
    disabled?: boolean;
  }

  interface Props {
    value?: string;
    options: Option[];
    name?: string;
    disabled?: boolean;
    required?: boolean;
    orientation?: "vertical" | "horizontal";
    onValueChange?: (value: string) => void;
  }

  let {
    value = $bindable(""),
    options,
    name,
    disabled = false,
    required = false,
    orientation = "vertical",
    onValueChange,
  }: Props = $props();

  const ff = getFormField();
  const groupId = `rg-${Math.random().toString(36).slice(2)}`;
</script>

<RadioGroup.Root
  bind:value
  {name}
  {disabled}
  {required}
  {orientation}
  {onValueChange}
  aria-describedby={ff?.describedby}
  class="flex {orientation === 'horizontal'
    ? 'flex-row flex-wrap gap-4'
    : 'flex-col gap-2'}"
>
  {#each options as opt, i (opt.value)}
    {@const itemId = `${groupId}-${i}`}
    {@const isDisabled = opt.disabled || disabled}
    <div class="flex items-center gap-2 {isDisabled ? 'opacity-50' : ''}">
      <RadioGroup.Item
        value={opt.value}
        id={itemId}
        disabled={opt.disabled}
        class="radio-dot {isDisabled ? 'cursor-not-allowed' : 'cursor-pointer'}"
      />
      <label
        for={itemId}
        class="text-sm text-base-content select-none {isDisabled
          ? 'cursor-not-allowed'
          : 'cursor-pointer'}">{opt.label}</label
      >
    </div>
  {/each}
</RadioGroup.Root>
