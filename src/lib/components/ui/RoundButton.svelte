<script lang="ts">
  import type { Icon as IconType } from '@lucide/svelte';

  export let label: string;
  export let icon: typeof IconType | undefined = undefined;
  export let iconSize: number = 16;
  export let bgColor: string = 'bg-[#5661f6]';
  export let hoverColor: string = 'hover:opacity-90';
  export let textColor: string = 'text-white';
  export let size: string = 'h-9';
  export let rounded: string = 'rounded-full';
  export let fontSize: string = 'text-[16px]';
  export let disabled: boolean = false;
  export let customClass: string = '';

  function handleClick(event: MouseEvent) {
    if (!disabled) {
      // 触发父组件的点击事件
      const clickEvent = new CustomEvent('click', { detail: event });
      event.currentTarget?.dispatchEvent(clickEvent);
    }
  }
</script>

<button
  class="flex-1 {size} {bgColor} {hoverColor} {textColor} {rounded} {fontSize} flex items-center justify-center gap-1.5 transition-colors {customClass}"
  class:opacity-50={disabled}
  class:cursor-not-allowed={disabled}
  on:click={handleClick}
  {disabled}
>
  {#if icon}
    {@const Icon = icon}
    <Icon size={iconSize} />
  {/if}
  {label}
</button>
