<script lang="ts">
  import type { Icon as IconType } from "@lucide/svelte";
  import { Edit, Trash2 } from '@lucide/svelte';

  interface Props {
    title?: string;
    isActive?: boolean;
    icon?: typeof IconType | undefined;
    iconPosition?: "left" | "right";
    iconSize?: number;
    onClick?: () => void;
    enableContextMenu?: boolean;
    onRename?: ((newName: string) => void) | undefined;
    onDelete?: (() => void) | undefined;
    buttonClass?: string;
    activeClass?: string;
    iconClass?: string;
  }

  let {
    title = "",
    isActive = false,
    icon = undefined,
    iconPosition = "left",
    iconSize = 16,
    onClick = () => {},
    enableContextMenu = false,
    onRename = undefined,
    onDelete = undefined,
    buttonClass = "",
    activeClass = "",
    iconClass = ""
  }: Props = $props();

  // 默认样式
  const defaultButtonClass = "w-full p-2 text-left rounded-lg text-[14px] leading-[22px] text-gray-700 hover:bg-bg-hover truncate";
  const defaultActiveClass = "bg-bg-hover";
  const defaultIconClass = "flex-shrink-0";

  // 优化样式计算，避免频繁字符串拼接
  let finalButtonClass = $derived.by(() => {
    const activeClasses = isActive
      ? `${defaultActiveClass} ${activeClass}`
      : "";
    return [defaultButtonClass, activeClasses, buttonClass]
      .filter(Boolean)
      .join(" ");
  });

  let finalIconClass = $derived.by(() => {
    return [defaultIconClass, iconClass].filter(Boolean).join(" ");
  });

  // 检查是否有图标
  let hasIcon = $derived(icon !== undefined);

  // 右键菜单状态
  let showContextMenu = $state(false);
  let contextMenuX = $state(0);
  let contextMenuY = $state(0);
  let isRenaming = $state(false);
  let renameValue = $state(title);

  // 处理右键点击
  function handleContextMenu(event: MouseEvent) {
    if (!enableContextMenu) return;

    event.preventDefault();
    event.stopPropagation();

    contextMenuX = event.clientX;
    contextMenuY = event.clientY;
    showContextMenu = true;
  }

  // 处理重命名
  function handleRename() {
    isRenaming = true;
    renameValue = title;
    showContextMenu = false;
  }

  // 确认重命名
  function confirmRename() {
    if (onRename && renameValue.trim() !== title && renameValue.trim()) {
      onRename(renameValue.trim());
    }
    isRenaming = false;
  }

  // 取消重命名
  function cancelRename() {
    isRenaming = false;
    renameValue = title;
  }

  // 处理删除
  function handleDelete() {
    if (onDelete) {
      onDelete();
    }
    showContextMenu = false;
  }

  // 键盘事件处理
  function handleKeydown(event: KeyboardEvent) {
    if (isRenaming) {
      if (event.key === 'Enter') {
        confirmRename();
      } else if (event.key === 'Escape') {
        cancelRename();
      }
    }
  }

  // 点击外部关闭菜单
  function handleClickOutside() {
    showContextMenu = false;
  }

</script>

<!-- 主体部分 -->
{#if isRenaming}
  <!-- 重命名输入框 -->
  <div class="relative">
    <input
      class="w-full p-2 text-[14px] bg-white border border-gray-300 rounded-lg"
      bind:value={renameValue}
      onkeydown={handleKeydown}
      onblur={confirmRename}
      placeholder="输入新名称"
      autofocus
    />
  </div>
{:else}
  <!-- 普通按钮 -->
  <button
    class={finalButtonClass}
    onclick={onClick}
    oncontextmenu={handleContextMenu}
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
{/if}

<!-- 右键菜单 -->
{#if showContextMenu}
  <div
    class="fixed z-[10030] bg-white border border-gray-200 rounded-lg shadow-lg py-1 min-w-32"
    style="left: {contextMenuX}px; top: {contextMenuY}px;"
  >
    {#if onRename}
      <button
        class="w-full px-3 py-2 text-left text-[13px] hover:bg-gray-100 flex items-center gap-2"
        onclick={handleRename}
      >
        <Edit size={14} />
        重命名
      </button>
    {/if}
    {#if onDelete}
      <button
        class="w-full px-3 py-2 text-left text-[13px] hover:bg-gray-100 text-red-600 flex items-center gap-2"
        onclick={handleDelete}
      >
        <Trash2 size={14} />
        删除
      </button>
    {/if}
  </div>
{/if}

<!-- 点击外部关闭菜单 -->
<svelte:window onclick={handleClickOutside} />
