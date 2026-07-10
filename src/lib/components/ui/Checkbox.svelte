<script lang="ts">
  import { Checkbox } from "bits-ui";
  import { Check, Minus } from "@lucide/svelte";
  import type { Snippet } from "svelte";
  import { getFormField } from "./FormField.svelte";

  interface Props {
    checked?: boolean;
    indeterminate?: boolean;
    disabled?: boolean;
    required?: boolean;
    name?: string;
    value?: string;
    onCheckedChange?: (checked: boolean) => void;
    // 复选框右侧的文本标签。
    children?: Snippet;
  }

  let {
    checked = $bindable(false),
    indeterminate = $bindable(false),
    disabled = false,
    required = false,
    name,
    value,
    onCheckedChange,
    children,
  }: Props = $props();

  // 在 FormField 内时借用其 aria-describedby（error / hint）；id 不接管——一个字段可含多个
  // 复选框，各自用行内 label 承接点击。
  const ff = getFormField();
</script>

<label
  class="inline-flex items-center gap-2 text-sm text-base-content select-none {disabled
    ? 'cursor-not-allowed opacity-50'
    : 'cursor-pointer'}"
>
  <Checkbox.Root
    bind:checked
    bind:indeterminate
    {disabled}
    {required}
    {name}
    {value}
    {onCheckedChange}
    aria-describedby={ff?.describedby}
    class="checkbox-box"
  >
    {#snippet children({ checked: c, indeterminate: ind })}
      {#if ind}
        <Minus size={12} strokeWidth={3} />
      {:else if c}
        <Check size={12} strokeWidth={3} />
      {/if}
    {/snippet}
  </Checkbox.Root>
  {#if children}<span>{@render children()}</span>{/if}
</label>
