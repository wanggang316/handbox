<script lang="ts">
  export let value = '';
  export let items: Array<{ value: string; label: string }>= [];
  export let onChange: (val: string) => void = () => {};
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
.tabs { display: flex; border-bottom: 1px solid var(--border-color); margin-bottom: 1rem; }
.tab { padding: .75rem 1rem; background: none; border: none; border-bottom: 2px solid transparent; color: var(--text-secondary); cursor: pointer; }
.tab:hover { color: var(--text-primary); }
.tab.active { color: var(--bg-accent); border-bottom-color: var(--bg-accent); }
</style>


