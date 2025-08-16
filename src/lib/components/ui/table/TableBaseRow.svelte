<script lang="ts">
  interface Props {
    label?: string;
    layout?: 'horizontal' | 'vertical';
    rightContent?: any; // 标题行右边的内容
    children?: any;
  }

  let { label, layout = 'horizontal', rightContent, children }: Props = $props();
</script>

<div class="px-6 py-4">
  {#if label}
    {#if layout === 'vertical'}
      <div class="space-y-3">
        <div class="flex items-center justify-between">
          <div class="text-sm font-medium text-gray-700">{label}</div>
          {#if rightContent}
            <div>
              {@render rightContent?.()}
            </div>
          {/if}
        </div>
        <div>
          {@render children?.()}
        </div>
      </div>
    {:else}
      <div class="flex items-center justify-between">
        <div class="text-sm font-medium text-gray-700">{label}</div>
        <div class="ml-4">
          {@render children?.()}
        </div>
      </div>
    {/if}
  {:else}
    {@render children?.()}
  {/if}
</div>
