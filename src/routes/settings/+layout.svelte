<script lang="ts">
  import "../../app.css";
  import SettingsSidebar from "$lib/components/settings/SettingsSidebar.svelte";
  let { children } = $props();
</script>

<div class="flex h-screen bg-[color:var(--bg-primary)]">
  <SettingsSidebar />
  <main class="flex-1 overflow-auto">
    {@render children()}
  </main>
</div>
