<script lang="ts">
  import { onMount } from "svelte";
  import { ChevronRight } from "@lucide/svelte";
  import { openSettingsWindow } from "$lib/api/window";
  import {
    disableCurrentAppByPid,
    hideSettingsPanel,
    showSettingsDisablePanel,
  } from "$lib/api/selection";
  import { settingsState } from "$lib/states";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  const appWindow = getCurrentWindow();

  onMount(() => {
    settingsState.loadSettings().catch((error) => {
      console.error("加载设置失败:", error);
    });
  });

  async function handleHideUntilRestart() {
    await disableCurrentAppByPid();
    await hideSettingsPanel();
  }

  async function handleShowDisableSubmenu() {
    const [position, size, scale] = await Promise.all([
      appWindow.outerPosition(),
      appWindow.outerSize(),
      appWindow.scaleFactor(),
    ]);
    const logicalX = position.x / scale;
    const logicalY = position.y / scale;
    const logicalWidth = size.width / scale;
    const x = logicalX + logicalWidth;
    const y = logicalY + 38;
    await showSettingsDisablePanel(x, y);
  }

  async function handleOpenSettings() {
    await openSettingsWindow("quicktools");
    await hideSettingsPanel();
  }
</script>

<div class="w-full h-full bg-red-500">
  <div class="flex flex-col bg-white gap-1">
    <button
      class="flex items-center w-full px-3 py-2 text-sm text-gray-700 hover:bg-red-200"
      onclick={handleHideUntilRestart}
    >
      隐藏至重启此应用
    </button>

    <button
      class="flex items-center justify-between w-full px-3 py-2 text-sm text-gray-700 hover:bg-gray-100"
      onclick={handleShowDisableSubmenu}
    >
      <span>禁用</span>
      <ChevronRight class="size-3.5 text-gray-400" />
    </button>

    <button
      class="flex items-center w-full px-3 py-2 text-sm text-gray-700 hover:bg-gray-100"
      onclick={handleOpenSettings}
    >
      设置
    </button>
  </div>
</div>
