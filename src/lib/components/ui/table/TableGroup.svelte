<script lang="ts">
  import { slide } from "svelte/transition";
  import { ChevronDown } from "@lucide/svelte";

  interface Props {
    title?: string;
    collapsible?: boolean;
    defaultCollapsed?: boolean;
    showDivider?: boolean;
    children?: any;
  }

  let {
    title,
    collapsible = false,
    defaultCollapsed = false,
    showDivider = true,
    children,
  }: Props = $props();

  let isCollapsed = $state(defaultCollapsed);
  let isHovering = $state(false);
  // 折叠动画仅在用户实际点击后启用：组件挂载也会播放 intro transition，
  // 若一开始就带时长，切换设置页时所有分组卡都会滑入一遍（观感拖沓卡顿）。
  let hasToggled = $state(false);

  function toggleCollapse() {
    if (collapsible) {
      hasToggled = true;
      isCollapsed = !isCollapsed;
    }
  }
</script>

<div class="flex flex-col {title ? 'pt-2' : ''}">
  {#if title}
    <button
      type="button"
      class="flex items-center justify-between mb-2 mx-1 text-sm font-medium {collapsible
        ? 'cursor-pointer text-base-content/90 hover:text-base-content'
        : 'text-base-content cursor-default'}"
      onclick={toggleCollapse}
      onmouseenter={() => (isHovering = true)}
      onmouseleave={() => (isHovering = false)}
      disabled={!collapsible}
    >
      <span>{title}</span>
      {#if collapsible}
        <!-- 常驻单 chevron：hover 淡入、展开时旋转指上（替代双 icon 切换） -->
        <ChevronDown
          size={16}
          class="transition duration-150 {isHovering
            ? 'opacity-100'
            : 'opacity-0'} {isCollapsed ? '' : 'rotate-180'}"
        />
      {/if}
    </button>
  {/if}

  {#if !collapsible || !isCollapsed}
    <div
      transition:slide={{ duration: hasToggled ? 160 : 0 }}
      class="table-group bg-[var(--bg-panel)] rounded-xl border border-[var(--hairline)] overflow-hidden {showDivider
        ? 'show-divider'
        : ''}"
    >
      {@render children?.()}
    </div>
  {/if}
</div>

<style>
  .table-group.show-divider :global(> *:not(:last-child)) {
    position: relative;
  }

  .table-group.show-divider :global(> *:not(:last-child)::after) {
    content: "";
    position: absolute;
    bottom: 0;
    left: 1.5rem;
    right: 1.5rem;
    height: 1px;
    background-color: var(--hairline);
  }
</style>
