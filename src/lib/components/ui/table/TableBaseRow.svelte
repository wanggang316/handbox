<script lang="ts">
  interface Props {
    label?: string;
    icon?: any; // Lucide图标组件
    layout?: "horizontal" | "vertical";
    py?: string;
    rightContent?: any; // 标题行右边的内容
    children?: any;
  }

  let {
    label,
    icon,
    layout = "horizontal",
    py = "4",
    rightContent,
    children,
  }: Props = $props();
</script>

<div class="px-6 py-{py}">
  {#if label}
    {#if layout === "vertical"}
      <div class="space-y-3">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            {#if icon}
              {@render icon({ class: "w-4 h-4 text-base-content/70" })}
            {/if}
            <div class="text-sm text-base-content">{label}</div>
          </div>
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
        <div class="flex items-center gap-2">
          {#if icon}
            {@render icon({ class: "w-4 h-4 text-base-content/70" })}
          {/if}
          <div class="text-sm text-base-content">{label}</div>
        </div>
        <div class="flex justify-end flex-1 ml-4">
          {@render children?.()}
        </div>
      </div>
    {/if}
  {:else}
    {@render children?.()}
  {/if}
</div>
