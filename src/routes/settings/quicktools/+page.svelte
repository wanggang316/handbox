<script lang="ts">
  import { onMount } from "svelte";
  import { TableGroup, SwitchRow } from "$lib/components/ui/table";
  import { settingsState } from "$lib/states";
  import { ExternalLink } from "@lucide/svelte";
  import {
    checkAccessibilityPermission,
    requestAccessibilityPermission,
    openAccessibilitySettings,
  } from "$lib/api";

  let showToolbarOnSelection: boolean = false;
  let permissionGranted: boolean = false;
  let isCheckingPermission: boolean = false;

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
    } catch (error) {
      console.error("加载快捷工具设置失败:", error);
    }
  });

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
</script>

<div class="mt-8 p-6 pr-8 flex flex-col gap-y-4">
  <TableGroup>
    <SwitchRow
      label="选中文本显示工具栏"
      bind:checked={showToolbarOnSelection}
      description={permissionGranted ? "" : "需要辅助功能权限"}
      disabled={isCheckingPermission}
      onChange={handleToggleChange}
    />
  </TableGroup>

  {#if !permissionGranted}
    <div class="bg-base-200 rounded-xl p-4 flex flex-col gap-3">
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
