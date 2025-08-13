<script lang="ts">
  import type { Icon as IconType } from '@lucide/svelte';
  
  export let title: string = "";
  export let isActive: boolean = false;
  export let icon: typeof IconType | undefined = undefined;
  export let iconPosition: "left" | "right" = "left";
  export let iconSize: number = 16;
  export let onClick: () => void = () => {};
  
  // 自定义样式类
  export let buttonClass: string = "";
  export let activeClass: string = "";
  export let iconClass: string = "";
  
  // 默认样式
  const defaultButtonClass = "w-full px-2 py-2 text-left rounded-md text-[16px] leading-[22px] text-black hover:bg-[#EDEDED] transition-colors truncate";
  const defaultActiveClass = "bg-[#EDEDED]";
  const defaultIconClass = "w-4 h-4 flex-shrink-0";
  
  $: finalButtonClass = `${defaultButtonClass} ${isActive ? defaultActiveClass + ' ' + activeClass : ''} ${buttonClass}`;
  $: finalIconClass = `${defaultIconClass} ${iconClass}`;
  
  // 检查是否有图标
  $: hasIcon = icon || $$slots.icon;
</script>

<button
  class={finalButtonClass}
  on:click={onClick}
>
  <div class="flex items-center gap-2">
    {#if hasIcon && iconPosition === "left"}
      <div class={finalIconClass}>
        {#if icon}
          {@const Icon = icon}
          <Icon size={iconSize} />
        {:else}
          <slot name="icon" />
        {/if}
      </div>
    {/if}
    
    <span class="truncate">{title}</span>
    
    {#if hasIcon && iconPosition === "right"}
      <div class={finalIconClass}>
        {#if icon}
          {@const Icon = icon}
          <Icon size={iconSize} />
        {:else}
          <slot name="icon" />
        {/if}
      </div>
    {/if}
  </div>
</button>
