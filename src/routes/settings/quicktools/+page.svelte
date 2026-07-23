<script lang="ts" module>
  import type { DisabledApp as DisabledAppCache } from "$lib/api/selection";

  // 跨挂载缓存：权限探测与禁用应用列表都是异步的，缓存上次结果让重访首帧直出，
  // 异步刷新静默纠偏。null 表示从未探测过（本会话首次）。
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
  // 划词「翻译」使用的 Agent 定义 ID；"" = 内置翻译回落
  let translationAgentId = $state("");
  // Quick Action（全局快捷键唤起浮层）是否启用；缺省视为 true
  let quickActionEnabled = $state(true);
  // 权限/禁用应用是异步探测的：用模块级缓存让重访首帧直出上次结果，避免
  // 「警告条闪现又消失」「列表先空后填」。首次访问乐观按已授权画（警告延迟出现
  // 好过对多数已授权用户闪一下警告）。
  let permissionGranted = $state(cachedPermission ?? true);
  let isCheckingPermission = $state(false);
  let disabledApps = $state<DisabledApp[]>(cachedApps ?? []);
  let isLoadingApps = $state(false);

  // 从 settings 回填本地状态；store 未就绪时跳过
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

  // 根布局已预加载 settings：同步回填，首帧即真实值，避免开关闪烁
  syncFromSettings();

  onMount(async () => {
    try {
      // 兜底冷启动/深链：确保 settings 加载完成后再同步一次
      await settingsState.loadSettings();
      syncFromSettings();

      // 检查当前权限状态
      permissionGranted = await checkAccessibilityPermission();
      cachedPermission = permissionGranted;
      console.log("[QuickTools] 初始化: permissionGranted =", permissionGranted);

      // 加载禁用的应用列表
      await loadDisabledApps();
    } catch (error) {
      console.error("加载快捷工具设置失败:", error);
    }

    // 翻译 Agent 选择器的候选列表；失败不阻塞页面其余部分
    try {
      await agentActions.loadAgents();
    } catch (error) {
      console.error("加载 Agent 列表失败:", error);
    }
  });

  // 翻译 Agent 下拉候选："" = 内置翻译回落，其余为全部 Agent 定义
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
   * 切换 Quick Action：先持久化 enabled，再注册/反注册全局快捷键，保持
   * 「开关状态 = 热键是否生效」的不变量；注册失败则回滚开关与持久化值。
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
    // 有缓存时静默刷新（不闪 spinner），仅冷启动首次显示加载态。
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
      // 用户尝试开启功能
      isCheckingPermission = true;
      try {
        // 请求权限，会自动弹出系统授权提示
        console.log("[QuickTools] 调用 requestAccessibilityPermission...");
        const granted = await requestAccessibilityPermission();
        console.log("[QuickTools] requestAccessibilityPermission 返回:", granted);
        permissionGranted = granted;
        cachedPermission = granted;

        if (granted) {
          // 权限已授予，保存设置
          showToolbarOnSelection = true;
          await settingsState.updateSettings({
            section: "quickTools",
            data: { showToolbarOnSelection: true },
          });
        } else {
          // 权限未授予，保持关闭状态，并打开系统设置
          showToolbarOnSelection = false;
          // 如果系统弹窗没有出现，主动打开设置页面
          await openAccessibilitySettings();
        }
      } catch (error) {
        console.error("检查辅助功能权限失败:", error);
        showToolbarOnSelection = false;
      } finally {
        isCheckingPermission = false;
      }
    } else {
      // 用户关闭功能 - 无需检查权限
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
    // 如果权限已授予且之前尝试开启过，自动开启功能
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
      // 重新加载列表
      await loadDisabledApps();
    } catch (error) {
      console.error("移除禁用应用失败:", error);
    }
  }
</script>

<div class="p-6 pr-8 pt-2 flex flex-col gap-y-6">
  <!-- 划词工具栏：显示开关 + 翻译 Agent + 禁用的应用列表 -->
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

    <!-- 禁用的应用列表（包成单个子元素，避免组内分隔线切开标题与列表） -->
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

  <!-- Quick Action（全局快捷键唤起浮层） -->
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
