<script lang="ts">
  interface Props {
    value?: string;
    items?: Array<{ value: string; label: string }>;
    onChange?: (val: string) => void;
  }

  let { value = '', items = [], onChange = () => {} }: Props = $props();
</script>

<div class="tabs" role="tablist">
  {#each items as item (item.value)}
    <button
      role="tab"
      class="tab"
      class:active={value === item.value}
      aria-selected={value === item.value}
      onclick={() => onChange(item.value)}
    >{item.label}</button>
  {/each}
</div>

<style>
.tabs { display: flex; border-bottom: 1px solid var(--base-300); margin-bottom: 1rem; }
.tab { padding: .75rem 1rem; background: none; border: none; border-bottom: 2px solid transparent; color: color-mix(in oklch, var(--base-content) 80%, transparent); cursor: pointer; }
.tab:hover { color: var(--base-content); }
.tab.active { color: var(--primary); border-bottom-color: var(--primary); }
</style>


