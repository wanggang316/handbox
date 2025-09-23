<script lang="ts">
  import Textarea from '../Textarea.svelte';
  import TableBaseRow from './TableBaseRow.svelte';

  interface Props {
    label?: string;
    value: string;
    placeholder?: string;
    rows?: number;
    disabled?: boolean;
    readonly?: boolean;
    maxlength?: number;
    minlength?: number;
    required?: boolean;
    showCharCount?: boolean;
    description?: string;
    rightContent?: any;
  }

  let {
    label = '',
    value = $bindable(),
    placeholder = '',
    rows = 4,
    disabled = false,
    readonly = false,
    maxlength,
    minlength,
    required = false,
    showCharCount = false,
    description = '',
    rightContent
  }: Props = $props();
</script>

<TableBaseRow {label} {rightContent} layout="vertical">
  <div class="space-y-2">
    <Textarea 
      bind:value
      {placeholder}
      {rows}
      {disabled}
      {readonly}
      {maxlength}
      {minlength}
      {required}
      {showCharCount}
    />
    
    {#if description}
      <div class="text-xs text-base-content/70">
        {description}
      </div>
    {/if}
  </div>
</TableBaseRow>
