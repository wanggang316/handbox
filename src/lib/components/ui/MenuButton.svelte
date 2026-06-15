<script lang="ts">
  import type { Icon as IconType } from "@lucide/svelte";
  import type { Snippet } from "svelte";

  interface Props {
    title?: string;
    isActive?: boolean;
    icon?: typeof IconType;
    iconPosition?: "left" | "right";
    iconSize?: number;
    onclick?: () => void;
    // 自定义样式类
    buttonClass?: string;
    activeClass?: string;
    iconClass?: string;
    // 图标插槽（无 icon prop 时回退）
    icon_slot?: Snippet;
  }

  let {
    title = "",
    isActive = false,
    icon = undefined,
    iconPosition = "left",
    iconSize = 16,
    onclick,
    buttonClass = "",
    activeClass = "",
    iconClass = "",
    icon_slot,
  }: Props = $props();

  // 默认样式
  const defaultButtonClass =
    "w-full p-2 text-left rounded-lg text-[14px] leading-[22px] text-base-content hover:bg-base-300 truncate";
  const defaultActiveClass = "bg-base-300";
  const defaultIconClass = "flex-shrink-0";

  // 优化样式计算，避免频繁字符串拼接
  const finalButtonClass = $derived(
    [
      defaultButtonClass,
      isActive ? `${defaultActiveClass} ${activeClass}` : "",
      buttonClass,
    ]
      .filter(Boolean)
      .join(" "),
  );
  const finalIconClass = $derived(
    [defaultIconClass, iconClass].filter(Boolean).join(" "),
  );

  // 检查是否有图标
  const hasIcon = $derived(Boolean(icon || icon_slot));
</script>

<button class={finalButtonClass} {onclick}>
  <div class="flex items-center gap-2">
    {#if hasIcon && iconPosition === "left"}
      <div class={finalIconClass}>
        {#if icon}
          {@const Icon = icon}
          <Icon size={iconSize} />
        {:else}
          {@render icon_slot?.()}
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
          {@render icon_slot?.()}
        {/if}
      </div>
    {/if}
  </div>
</button>
