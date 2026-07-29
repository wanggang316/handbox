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
  // Enable the collapse animation only after a real toggle: mount also plays the
  // intro transition, so a nonzero duration would make every group card slide in
  // when switching settings pages (sluggish feel).
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
        <!-- Single persistent chevron: fades in on hover, rotates to point up when expanded. -->
        <ChevronDown
          size={16}
          class="transition duration-[var(--dur-fast)] {isHovering
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
