<script lang="ts" module>
  import type { DisabledApp as DisabledAppCache } from "$lib/api/selection";

  // Cross-mount cache: permission probe and disabled-app list are async; cache
  // the last results so revisits paint immediately and refresh silently.
  // null = never probed this session.
  let cachedPermission: boolean | null = null;
  let cachedApps: DisabledAppCache[] | null = null;
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { TableGroup, SwitchRow, SelectRow } from "$lib/components/ui/table";
  import { settingsState } from "$lib/states";
  import { agentState, agentActions } from "$lib/states/agent.svelte";
  import { DEFAULT_ACCELERATOR } from "$lib/quickaction/accelerator";
  import { t } from "$lib/i18n";
  import { ExternalLink, RefreshCcw, Trash2, X } from "@lucide/svelte";
  import {
    checkAccessibilityPermission,
    getDisabledApps,
    removeDisabledApp,
    requestAccessibilityPermission,
    openAccessibilitySettings,
  } from "$lib/api";

  import type { DisabledApp } from "$lib/api/selection";
    import Button from "$lib/components/ui/Button.svelte";

  let showToolbarOnSelection = $state(false);
  // Agent definition id for selection "translate"; "" = builtin translation fallback
  let translationAgentId = $state("");
  // Quick Action (global-shortcut overlay); a missing setting defaults to true
  let quickActionEnabled = $state(true);
  // Permission / disabled apps are probed async: the module-level cache lets
  // revisits paint the last result immediately (no warning flash, no empty-then-
  // filled list). First visit optimistically assumes granted — a late warning
  // beats flashing one at every already-granted user.
  let permissionGranted = $state(cachedPermission ?? true);
  let isCheckingPermission = $state(false);
  let disabledApps = $state<DisabledApp[]>(cachedApps ?? []);
  let isLoadingApps = $state(false);

  function syncFromSettings(): void {
    if (!settingsState.settings) return;
    if (settingsState.settings.quickTools) {
      showToolbarOnSelection =
        settingsState.settings.quickTools.showToolbarOnSelection;
      translationAgentId =
        settingsState.settings.quickTools.translationAgentId ?? "";
    }
    quickActionEnabled = settingsState.settings.quickAction?.enabled ?? true;
  }

  // Root layout preloaded settings: sync backfill so the first frame shows real
  // values (no toggle flicker).
  syncFromSettings();

  onMount(async () => {
    try {
      // Cold-start/deep-link fallback: resync once settings finish loading
      await settingsState.loadSettings();
      syncFromSettings();

      permissionGranted = await checkAccessibilityPermission();
      cachedPermission = permissionGranted;
      console.log("[QuickTools] 初始化: permissionGranted =", permissionGranted);

      await loadDisabledApps();
    } catch (error) {
      console.error("加载快捷工具设置失败:", error);
    }

    // Candidates for the translation-agent picker; failure doesn't block the rest
    try {
      await agentActions.loadAgents();
    } catch (error) {
      console.error("加载 Agent 列表失败:", error);
    }
  });

  // "" = builtin translation fallback; the rest are all agent definitions
  const translationAgentOptions = $derived([
    { value: "", label: t("settings.quicktools.translationAgentDefault") },
    ...agentState.agents.flatMap((a) =>
      a.id ? [{ value: a.id, label: a.name }] : [],
    ),
  ]);

  async function handleTranslationAgentSelect(value: string) {
    try {
      await settingsState.updateSettings({
        section: "quickTools",
        data: { translationAgentId: value || null },
      });
    } catch (error) {
      console.error("更新划词翻译 Agent 失败:", error);
    }
  }

  /**
   * Toggle Quick Action: persist enabled first, then (un)register the global
   * shortcut so "switch state = hotkey active" holds; on registration failure
   * roll back both the switch and the persisted value.
   */
  async function handleQuickActionToggle(checked: boolean) {
    quickActionEnabled = checked;
    try {
      await settingsState.updateSettings({
        section: "quickAction",
        data: { enabled: checked },
      });
      if (checked) {
        const accelerator =
          settingsState.settings?.quickAction?.shortcut ?? DEFAULT_ACCELERATOR;
        await invoke("quick_action_register_shortcut", { accelerator });
      } else {
        await invoke("quick_action_unregister_shortcut");
      }
    } catch (error) {
      console.error("切换 Quick Action 失败:", error);
      quickActionEnabled = !checked;
      try {
        await settingsState.updateSettings({
          section: "quickAction",
          data: { enabled: !checked },
        });
      } catch (rollbackError) {
        console.error("回滚 Quick Action 开关失败:", rollbackError);
      }
    }
  }

  async function loadDisabledApps() {
    // Silent refresh when cached (no spinner); only the first cold load shows loading
    isLoadingApps = cachedApps === null;
    try {
      disabledApps = await getDisabledApps();
      cachedApps = disabledApps;
      console.log("[QuickTools] 禁用的应用:", disabledApps);
    } catch (error) {
      console.error("加载禁用应用列表失败:", error);
      disabledApps = [];
    } finally {
      isLoadingApps = false;
    }
  }

  async function handleToggleChange(checked: boolean) {
    console.log("[QuickTools] handleToggleChange:", checked);
    if (checked) {
      isCheckingPermission = true;
      try {
        // Requesting permission also pops the system authorization prompt
        console.log("[QuickTools] 调用 requestAccessibilityPermission...");
        const granted = await requestAccessibilityPermission();
        console.log("[QuickTools] requestAccessibilityPermission 返回:", granted);
        permissionGranted = granted;
        cachedPermission = granted;

        if (granted) {
          showToolbarOnSelection = true;
          await settingsState.updateSettings({
            section: "quickTools",
            data: { showToolbarOnSelection: true },
          });
        } else {
          showToolbarOnSelection = false;
          // If the system prompt didn't appear, open the settings pane directly
          await openAccessibilitySettings();
        }
      } catch (error) {
        console.error("检查辅助功能权限失败:", error);
        showToolbarOnSelection = false;
      } finally {
        isCheckingPermission = false;
      }
    } else {
      // Turning off needs no permission check
      showToolbarOnSelection = false;
      await settingsState.updateSettings({
        section: "quickTools",
        data: { showToolbarOnSelection: false },
      });
    }
  }

  async function handleOpenSettings() {
    await openAccessibilitySettings();
  }

  async function handleRefreshPermission() {
    permissionGranted = await checkAccessibilityPermission();
    cachedPermission = permissionGranted;
    // If permission is now granted, auto-enable the previously attempted switch
    if (permissionGranted && !showToolbarOnSelection) {
      showToolbarOnSelection = true;
      await settingsState.updateSettings({
        section: "quickTools",
        data: { showToolbarOnSelection: true },
      });
    }
  }

  async function handleRemoveApp(bundleId: string) {
    try {
      await removeDisabledApp(bundleId);
      await loadDisabledApps();
    } catch (error) {
      console.error("移除禁用应用失败:", error);
    }
  }
</script>

<div class="p-6 pr-8 pt-2 flex flex-col gap-y-6">
  <TableGroup title={t("settings.quicktools.selectionToolbarGroup")}>
    <SwitchRow
      label={t("settings.quicktools.showToolbarOnSelection")}
      bind:checked={showToolbarOnSelection}
      description={permissionGranted ? "" : t("settings.quicktools.permissionRequired")}
      disabled={isCheckingPermission}
      onChange={handleToggleChange}
    />
    <SelectRow
      label={t("settings.quicktools.translationAgent")}
      description={t("settings.quicktools.translationAgentDesc")}
      options={translationAgentOptions}
      bind:selectedValue={translationAgentId}
      onSelect={handleTranslationAgentSelect}
    />

    <!-- Wrapped in a single child so the group divider doesn't split heading and list -->
    <div>
      <div class="flex items-center justify-between px-6 py-4">
        <h3 class="text-sm text-base-content">{t("settings.quicktools.disabledApps")}</h3>
        {#if !isLoadingApps}
          <Button variant="clear" size="icon-sm" onclick={loadDisabledApps}>
            <RefreshCcw size={16} />
          </Button>
        {/if}
      </div>

      {#if isLoadingApps}
        <div class="flex justify-center py-8">
          <div class="text-sm text-base-content/50">{t("common.loading")}</div>
        </div>
      {:else if disabledApps.length === 0}
        <div class="flex justify-center py-8">
          <p class="text-sm text-base-content/50">
            {t("settings.quicktools.disabledAppsEmpty")}
          </p>
        </div>
      {:else}
        <div class="grid grid-cols-2 p-4 gap-x-4 gap-y-2">
          {#each disabledApps as app}
            <div class="flex items-center justify-between px-3 py-2 bg-base-300 rounded-lg group">
              <div class="flex flex-col">
                <span class="text-sm font-medium">{app.name}</span>
                <span class="text-xs text-base-content/50">{app.bundle_id}</span>
              </div>

              <Button
                variant="clear"
                size="icon-sm"
                onclick={() => handleRemoveApp(app.bundle_id)}
                title={t("common.remove")}
              >
                <Trash2 size={14} />
              </Button>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </TableGroup>

  <TableGroup title={t("settings.quicktools.quickActionGroup")}>
    <SwitchRow
      label={t("settings.quicktools.enableQuickAction")}
      bind:checked={quickActionEnabled}
      description={t("settings.quicktools.enableQuickActionDesc", {
        shortcut: settingsState.settings?.quickAction?.shortcut ?? DEFAULT_ACCELERATOR,
      })}
      onChange={handleQuickActionToggle}
    />
  </TableGroup>

  {#if !permissionGranted}
    <div class="bg-base-300 rounded-lg p-4 flex flex-col gap-3">
      <p class="text-sm text-base-content/70">
        {t("settings.quicktools.permissionGuide")}
      </p>
      <div class="flex items-center gap-2">
        <button
          class="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-lg bg-primary text-primary-content hover:bg-primary/90"
          onclick={handleOpenSettings}
        >
          <ExternalLink size={14} />
          {t("settings.quicktools.openSystemSettings")}
        </button>
        <button
          class="px-3 py-1.5 text-sm rounded-lg bg-base-300 text-base-content hover:bg-base-300/80"
          onclick={handleRefreshPermission}
        >
          {t("settings.quicktools.refreshPermission")}
        </button>
      </div>
    </div>
  {/if}
</div>
