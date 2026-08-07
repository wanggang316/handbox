<script lang="ts">
  import "../../app.css";
  import { page } from "$app/stores";
  import SettingsSidebar from "$lib/components/settings/SettingsSidebar.svelte";
  import TitleBar from "$lib/components/ui/TitleBar.svelte";
  import PageHeader from "$lib/components/ui/PageHeader.svelte";
  import { findSettingsNavItem } from "$lib/components/settings/settingsNav";

  let { children } = $props();

  // Shared header: resolve the title from the nav table (child routes map to
  // their top-level item).
  const currentTitle = $derived(
    findSettingsNavItem($page.url.pathname)?.title ?? "",
  );

  // Settings has no sidebar toggle; pass a no-op to satisfy TitleBar
  function handleToggle() {
  }
</script>

<!-- bg-sidebar: opaque --bg-page normally, transparent under macOS vibrancy -->
<div class="flex h-screen bg-[color:var(--bg-sidebar)]">
  <TitleBar sidebarOpen={false} showToggleButton={false} onToggle={handleToggle} />

  <div class="my-2 ml-2 w-56">
    <SettingsSidebar/>
  </div>

  <main
    class="flex-1 overflow-auto bg-[color:var(--bg-canvas)] border-l border-[var(--hairline)] rounded-tl-xl rounded-bl-xl"
  >
    <!-- Constrain the content column: settings don't stretch with the window -->
    <div class="mx-auto w-full max-w-3xl">
      {#if currentTitle}
        <div class="px-6 pb-2 pr-8 pt-14">
          <PageHeader title={currentTitle} />
        </div>
      {/if}
      {@render children()}
    </div>
  </main>
</div>
