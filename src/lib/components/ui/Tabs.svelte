<script lang="ts">
  interface Props {
    value?: string;
    items?: Array<{ value: string; label: string }>;
    onChange?: (val: string) => void;
  }

  let { value = '', items = [], onChange = () => {} }: Props = $props();
</script>

<!-- Codex 式分段 tab：选中为灰底胶囊，无下划线/底部边框 -->
<div class="flex items-center gap-1" role="tablist">
  {#each items as item (item.value)}
    <button
      role="tab"
      aria-selected={value === item.value}
      class="rounded-md px-2.5 py-1 text-sm transition-colors {value === item.value
        ? 'bg-base-200 font-medium text-base-content'
        : 'text-base-content/55 hover:text-base-content/85'}"
      onclick={() => onChange(item.value)}
    >{item.label}</button>
  {/each}
</div>
