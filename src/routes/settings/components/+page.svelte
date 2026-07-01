<script lang="ts">
  import Button from "$lib/components/ui/Button.svelte";
  import { t } from "$lib/i18n";

  const triggerAlert = () => {
    alert("Components demo page - under maintenance");
  };
</script>

<div class="p-6 space-y-4">
  <header class="space-y-2">
    <h1 class="text-xl font-medium text-base-content">UI 组件测试</h1>
    <p class="text-sm text-base-content/70">
      Component showcase - JSON UI renderer was removed during chat system cleanup.
    </p>
  </header>

  <section class="space-y-4">
    <h2 class="text-base font-medium text-base-content">Button Demo</h2>
    <div class="flex flex-wrap gap-2">
      <Button variant="primary">Primary</Button>
      <Button variant="secondary">Secondary</Button>
      <Button variant="gray">Gray</Button>
      <Button disabled>Disabled</Button>
    </div>
  </section>

  <section class="space-y-4 p-4 rounded border border-base-300">
    <p class="text-sm text-base-content/70">
      The JSON UI component previews were removed as they depended on utilities that were deleted during the chat system cleanup.
      This page can be updated to recreate those utilities or provide alternative component demonstrations.
    </p>
  </section>
</div>
