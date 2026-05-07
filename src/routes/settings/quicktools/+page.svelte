<script lang="ts">
  import { onMount } from "svelte";
  import { TableGroup, SwitchRow } from "$lib/components/ui/table";
  import { settingsState } from "$lib/states";
  import { ExternalLink, RefreshCcw, Trash2, X } from "@lucide/svelte";
  import {
    checkAccessibilityPermission,
    getDisabledApps,
    removeDisabledApp,
    requestAccessibilityPermission,
    openAccessibilitySettings,
  } from "$lib/api";

  import type { DisabledApp } from "$lib/api/selection";
    import IconButton from "$lib/components/ui/IconButton.svelte";

  let showToolbarOnSelection: boolean = false;
  let permissionGranted: boolean = false;
  let isCheckingPermission: boolean = false;
  let disabledApps: DisabledApp[] = [];
  let isLoadingApps: boolean = false;

  onMount(async () => {
    try {
      await settingsState.loadSettings();
      if (settingsState.settings?.quickTools) {
        showToolbarOnSelection =
          settingsState.settings.quickTools.showToolbarOnSelection;
      }

      // 检查当前权限状态
      permissionGranted = await checkAccessibilityPermission();
      console.log("[QuickTools] 初始化: permissionGranted =", permissionGranted);

      // 加载禁用的应用列表
      await loadDisabledApps();
    } catch (error) {
      console.error("加载快捷工具设置失败:", error);
    }
  });

  async function loadDisabledApps() {
    isLoadingApps = true;
    try {
      disabledApps = await getDisabledApps();
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

<div class="mt-8 p-6 pr-8 flex flex-col gap-y-6">
  <TableGroup>
    <SwitchRow
      label="选中文本显示工具栏"
      bind:checked={showToolbarOnSelection}
      description={permissionGranted ? "" : "需要辅助功能权限"}
      disabled={isCheckingPermission}
      onChange={handleToggleChange}
    />
  </TableGroup>

  <!-- 禁用的应用列表 -->
  <TableGroup>
    <div class="flex items-center justify-between px-6 py-4">
      <h3 class="text-sm text-base-content">禁用的应用</h3>
      {#if !isLoadingApps}
        <IconButton
          icon={RefreshCcw}
          iconSize={16}
          onclick={loadDisabledApps}
        />
      {/if}
    </div>

    {#if isLoadingApps}
      <div class="flex justify-center py-8">
        <div class="text-sm text-base-content/50">加载中...</div>
      </div>
    {:else if disabledApps.length === 0}
      <div class="flex justify-center py-8">
        <p class="text-sm text-base-content/50">
          禁止使用划词工具的应用将显示在这里。
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

            <IconButton
              icon={Trash2}
              iconSize={14}
              onclick={() => handleRemoveApp(app.bundle_id)}
              title="移除"
            />
          </div>
        {/each}
      </div>
    {/if}
  </TableGroup>

  {#if !permissionGranted}
    <div class="bg-base-300 rounded-lg p-4 flex flex-col gap-3">
      <p class="text-sm text-base-content/70">
        启用此功能需要授予辅助功能权限。请前往"系统设置 &gt; 隐私与安全性 &gt;
        辅助功能"，并启用 HandBox 的权限。
      </p>
      <div class="flex items-center gap-2">
        <button
          class="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-lg bg-primary text-primary-content hover:bg-primary/90"
          onclick={handleOpenSettings}
        >
          <ExternalLink size={14} />
          打开系统设置
        </button>
        <button
          class="px-3 py-1.5 text-sm rounded-lg bg-base-300 text-base-content hover:bg-base-300/80"
          onclick={handleRefreshPermission}
        >
          刷新权限状态
        </button>
      </div>
    </div>
  {/if}
</div>
