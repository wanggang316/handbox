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
  import { t } from "$lib/i18n";

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

<div
  class="w-full h-full p-1 bg-[var(--bg-card)] rounded-xl shadow-lg border border-[var(--hairline)] overflow-hidden"
>
  <div class="flex flex-col gap-1">
    <button
      class="flex items-center w-full px-3 py-2 text-sm rounded-lg text-base-content hover:bg-base-300 transition-colors duration-[var(--dur-fast)] ease-[var(--ease-out)]"
      onclick={handleHideUntilRestart}
    >
      {t("selection.hideUntilRestart")}
    </button>

    <button
      class="flex items-center w-full px-3 py-2 text-sm rounded-lg text-base-content hover:bg-base-300 transition-colors duration-[var(--dur-fast)] ease-[var(--ease-out)]"
      onclick={handleDisableByBundleId}
    >
      {t("selection.disableForApp")}
    </button>

    <button
      class="flex items-center w-full px-3 py-2 text-sm rounded-lg text-base-content hover:bg-base-300 transition-colors duration-[var(--dur-fast)] ease-[var(--ease-out)]"
      onclick={handleDisableGlobal}
    >
      {t("selection.disableGlobal")}
    </button>

    <button
      class="flex items-center w-full px-3 py-2 text-sm rounded-lg text-base-content hover:bg-base-300 transition-colors duration-[var(--dur-fast)] ease-[var(--ease-out)]"
      onclick={handleOpenSettings}
    >
      {t("common.settings")}
    </button>
  </div>
</div>
