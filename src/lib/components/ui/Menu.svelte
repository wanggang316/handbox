<script lang="ts">
  import MenuButton from "./MenuButton.svelte";
  import type { Icon as IconType } from '@lucide/svelte';
  
  export let title: String = "";

  export let items: Array<{
    id: string;
    title: string;
    icon?: typeof IconType;
    iconPosition?: "left" | "right";
    iconSize?: number;
  }> = [];
  
  export let activeId: string = "";

  export let onItemClick: (item: any) => void = () => {};
  
  // 自定义样式类
  export let containerClass = "";
  export let itemClass = "";
  export let activeItemClass = "";
</script>

<div class="flex flex-col {containerClass}">
  <!-- 固定标题 -->
  {#if title}
    <div class="text-sm text-base-content/70 pb-2 pl-4 flex-shrink-0">{title}</div>
  {/if}
  
  <!-- 可滚动的菜单项 -->
  <div class="flex-1 overflow-y-auto space-y-1 px-2">
    {#each items as item}
      <MenuButton
        title={item.title}
        isActive={item.id == activeId}
        icon={item.icon}
        iconPosition={item.iconPosition || "left"}
        iconSize={item.iconSize || 16}
        onClick={() => onItemClick(item)}
        buttonClass={itemClass}
        activeClass={activeItemClass}
      />
    {/each}
  </div>
</div>
