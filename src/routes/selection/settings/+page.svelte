<script lang="ts">
  import { onMount } from "svelte";
  import { openSettingsWindow } from "$lib/api/window";
  import {
    disableCurrentAppByPid,
    disableCurrentAppByBundleId,
    disableGlobalSelection,
    hideMenuPanel,
    hideSettingsPanel,
  } from "$lib/api/selection";
  import { settingsState } from "$lib/states";

  onMount(() => {
    settingsState.loadSettings().catch((error) => {
      console.error("加载设置失败:", error);
    });
  });

  async function handleHideUntilRestart() {
    await disableCurrentAppByPid();
    await hideSettingsPanel();
    await hideMenuPanel();
  }

  async function handleDisableByBundleId() {
    await disableCurrentAppByBundleId();
    await hideSettingsPanel();
    await hideMenuPanel();
  }

  async function handleDisableGlobal() {
    await disableGlobalSelection();
    await hideSettingsPanel();
    await hideMenuPanel();
  }

  async function handleOpenSettings() {
    await openSettingsWindow("quicktools");
    await hideSettingsPanel();
    await hideMenuPanel();
  }
</script>

<div class="w-full h-full bg-red-500">
  <div class="flex flex-col bg-white gap-1">
    <button
      class="flex items-center w-full px-3 py-2 text-sm text-gray-700 hover:bg-gray-100"
      onclick={handleHideUntilRestart}
    >
      隐藏至重启此应用
    </button>

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

    <button
      class="flex items-center w-full px-3 py-2 text-sm text-gray-700 hover:bg-gray-100"
      onclick={handleOpenSettings}
    >
      设置
    </button>
  </div>
</div>
