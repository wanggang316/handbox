<script lang="ts">
  import { ChevronUp, ChevronDown } from "@lucide/svelte";

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

  function toggleCollapse() {
    if (collapsible) {
      isCollapsed = !isCollapsed;
    }
  }
</script>

<div class="flex flex-col {title ? 'pt-2' : ''}">
  {#if title}
    <button
      type="button"
      class="flex items-center justify-between my-1 mx-2 text-xs {collapsible
        ? 'cursor-pointer text-base-content/80 hover:text-base-content'
        : 'text-base-content/80 cursor-default'}"
      onclick={toggleCollapse}
      onmouseenter={() => (isHovering = true)}
      onmouseleave={() => (isHovering = false)}
      disabled={!collapsible}
    >
      <span>{title}</span>
      {#if collapsible && isHovering}
        {#if isCollapsed}
          <ChevronDown size={16} />
        {:else}
          <ChevronUp size={16} />
        {/if}
      {/if}
    </button>
  {/if}

  {#if !collapsible || !isCollapsed}
    <div class="relative flex-1">
      <div
        class="absolute inset-0 bg-base-200 rounded-[20px] pointer-events-none"
      ></div>
      <div class="relative table-group rounded-[20px] {showDivider ? 'show-divider' : ''}">
        {@render children?.()}
      </div>
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
    background-color: var(--base-300);
  }
</style>
