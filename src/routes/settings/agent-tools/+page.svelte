<script lang="ts">
  import { onMount } from "svelte";
  import { TableGroup, SwitchRow } from "$lib/components/ui/table";
  import { settingsState } from "$lib/states";
  import { BUILTIN_TOOLS, BUILTIN_TOOL_IDS } from "$lib/constants/agentTools";
  import { t } from "$lib/i18n";

  // 全局默认启用的工具集（coding-agent 注册名）。无 agent 段时视为全开默认。
  let enabledTools = $state<string[]>([...BUILTIN_TOOL_IDS]);

  // 从 settings 回填本地状态；store 未就绪时跳过
  function syncFromSettings(): void {
    if (!settingsState.settings) return;
    enabledTools = settingsState.settings.agent?.defaultEnabledTools ?? [
      ...BUILTIN_TOOL_IDS,
    ];
  }

  // 根布局已预加载 settings：同步回填，首帧即真实值，避免开关闪烁
  syncFromSettings();

  // 兜底冷启动/深链：确保 settings 加载完成后再同步一次
  onMount(() => {
    settingsState
      .loadSettings()
      .then(syncFromSettings)
      .catch((error) => {
        console.error("加载 Agent 工具设置失败:", error);
      });
  });

  function isEnabled(toolId: string): boolean {
    return enabledTools.includes(toolId);
  }

  async function handleToggle(toolId: string, checked: boolean) {
    const next = checked
      ? enabledTools.includes(toolId)
        ? enabledTools
        : [...enabledTools, toolId]
      : enabledTools.filter((id) => id !== toolId);
    enabledTools = next;
    try {
      await settingsState.updateSettings({
        section: "agent",
        data: { defaultEnabledTools: next },
      });
    } catch (error) {
      console.error("更新 Agent 工具设置失败:", error);
    }
  }
</script>

<div class="p-6 pr-8 pt-2 flex flex-col gap-y-4">
  <div class="flex flex-col gap-y-1">
    <p class="text-sm text-base-content/60">
      {t("settings.agentTools.description")}
    </p>
  </div>

  <TableGroup>
    {#each BUILTIN_TOOLS as tool (tool.id)}
      <SwitchRow
        label={t(tool.labelKey)}
        checked={isEnabled(tool.id)}
        onChange={(checked) => handleToggle(tool.id, checked)}
      />
    {/each}
  </TableGroup>
</div>
