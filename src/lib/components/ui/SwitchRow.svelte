<script lang="ts">
  import Toggle from './Toggle.svelte';

  interface Props {
    label: string;
    checked: boolean;
    description?: string;
    disabled?: boolean;
  }

  let { 
    label,
    checked = $bindable(),
    description = '',
    disabled = false
  }: Props = $props();
</script>

<div class="flex items-center justify-between">
  <div>
    <label for="switch-{label}" class="text-sm font-medium text-gray-700">{label}</label>
    {#if description}
      <div class="text-xs text-gray-500 mt-1">
        {description}
      </div>
    {/if}
  </div>
  <Toggle id="switch-{label}" bind:checked {disabled} />
</div>
