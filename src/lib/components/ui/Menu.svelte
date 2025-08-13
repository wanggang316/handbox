<script lang="ts">
  import MenuButton from "./MenuButton.svelte";
  import type { Icon as IconType } from '@lucide/svelte';
  
  export let title: String = "";

  export let items: Array<{
    id: string;
    title: string;
    isActive?: boolean;
    icon?: typeof IconType;
    iconPosition?: "left" | "right";
    iconSize?: number;
  }> = [];
  
  export let onItemClick: (item: any) => void = () => {};
  
  // 自定义样式类
  export let containerClass = "";
  export let itemClass = "";
  export let activeItemClass = "";
</script>

<div class="flex-1 overflow-y-auto space-y-1 {containerClass}">
  <div class="text-sm text-gray-600 pb-2 pl-2">{title}</div>
  {#each items as item}
    <MenuButton
      title={item.title}
      isActive={item.isActive || false}
      icon={item.icon}
      iconPosition={item.iconPosition || "left"}
      iconSize={item.iconSize || 16}
      onClick={() => onItemClick(item)}
      buttonClass={itemClass}
      activeClass={activeItemClass}
    />
  {/each}
</div>
