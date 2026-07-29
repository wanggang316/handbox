<script lang="ts">
  import Button from "$lib/components/ui/Button.svelte";
  import { PanelLeft } from "@lucide/svelte";
  import { t } from "$lib/i18n";

  interface Props {
    sidebarOpen?: boolean;
    showToggleButton?: boolean;
    onToggle?: () => void;
    children?: import("svelte").Snippet;
  }

  let {
    sidebarOpen = true,
    showToggleButton = true,
    onToggle,
    children,
  }: Props = $props();

  function handleToggle() {
    onToggle?.();
  }
</script>

<div class="drag-region" data-tauri-drag-region>
  {#if showToggleButton}
    <div class="sidebar-toggle-button">
      <Button
        variant="clear"
        size="icon-sm"
        ariaLabel={sidebarOpen ? t("ui.hideSidebar") : t("ui.showSidebar")}
        onclick={handleToggle}
      >
        <PanelLeft size={15} strokeWidth={1.5} />
      </Button>
    </div>
  {/if}

  {@render children?.()}
</div>

<style>
  /* Window drag region for titleBarStyle: "Overlay". */
  .drag-region {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: 50px;
    z-index: 9999;
    user-select: none;
    -webkit-user-select: none;
    pointer-events: auto;
  }

  .sidebar-toggle-button {
    position: absolute;
    top: 11px;
    left: 100px; /* clears the system window buttons */
    pointer-events: auto;
    z-index: 10000;
    transition: opacity var(--dur-base) ease-in-out;
  }

  .sidebar-toggle-button:hover {
    opacity: 1;
  }
</style>
