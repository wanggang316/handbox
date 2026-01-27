<script lang="ts">
  import { onMount } from "svelte";
  import {
    disableCurrentAppByBundleId,
    disableGlobalSelection,
    hideSettingsDisablePanel,
  } from "$lib/api/selection";
  import { settingsState } from "$lib/states";

  onMount(() => {
    settingsState.loadSettings().catch((error) => {
      console.error("加载设置失败:", error);
    });
  });

  async function handleDisableByBundleId() {
    await disableCurrentAppByBundleId();
    await hideSettingsDisablePanel();
  }

  async function handleDisableGlobal() {
    await disableGlobalSelection();
    await hideSettingsDisablePanel();
  }
</script>

<div class="flex flex-col w-full h-full bg-blue-500 gap-1">
  <button
    class="flex items-center w-full px-3 py-2 text-sm text-gray-700 hover:bg-gray-100"
    onclick={handleDisableByBundleId}
  >
    在此应用禁用
  </button>
  <button
    class="flex items-center w-full px-3 py-2 text-sm text-gray-700 hover:bg-gray-100"
    onclick={handleDisableGlobal}
  >
    全局禁用
  </button>
</div>
