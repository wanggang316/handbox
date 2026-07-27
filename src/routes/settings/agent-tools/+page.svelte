<script lang="ts">
  import { onMount } from "svelte";
  import { TableGroup, SwitchRow, SelectRow, TextRow } from "$lib/components/ui/table";
  import { settingsState } from "$lib/states";
  import { BUILTIN_TOOLS, BUILTIN_TOOL_IDS } from "$lib/constants/agentTools";
  import { t } from "$lib/i18n";

  // 全局默认启用的工具集（coding-agent 注册名）。无 agent 段时视为全开默认。
  let enabledTools = $state<string[]>([...BUILTIN_TOOL_IDS]);

  // web_search 搜索服务商配置（settings.agent.webSearch）。
  let webSearchProvider = $state("tavily");
  let webSearchApiKey = $state("");
  // 最近一次已持久化（或回填）的 webSearch 快照 —— 防抖 $effect 据此跳过
  // 回填触发的伪变更，只在用户真实编辑后写盘。
  let webSearchPersisted = $state("");

  const webSearchProviderOptions = [{ value: "tavily", label: "Tavily" }];

  // 分组展示：第一组只列 coding-agent 内置工具；web_search 与其服务商配置同组；
  // 其余扩展工具（render_card / render_app / skill）在「扩展工具」组，各带一行说明。
  const EXTENSION_IDS = ["web_search", "render_card", "render_app", "skill"];
  const codingAgentTools = BUILTIN_TOOLS.filter(
    (tool) => !EXTENSION_IDS.includes(tool.id),
  );
  const extensionTools = $derived([
    { id: "render_card", label: t("agent.tool.render_card"), desc: t("settings.agentTools.renderCardDesc") },
    { id: "render_app", label: t("agent.tool.render_app"), desc: t("settings.agentTools.renderAppDesc") },
    { id: "skill", label: t("agent.tool.skill"), desc: t("settings.agentTools.skillDesc") },
  ]);

  function webSearchSnapshot(provider: string, apiKey: string): string {
    return JSON.stringify({ provider, apiKey });
  }

  // 从 settings 回填本地状态；store 未就绪时跳过
  function syncFromSettings(): void {
    if (!settingsState.settings) return;
    enabledTools = settingsState.settings.agent?.defaultEnabledTools ?? [
      ...BUILTIN_TOOL_IDS,
    ];
    const webSearch = settingsState.settings.agent?.webSearch;
    webSearchProvider = webSearch?.provider ?? "tavily";
    webSearchApiKey = webSearch?.apiKey ?? "";
    webSearchPersisted = webSearchSnapshot(webSearchProvider, webSearchApiKey);
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

  // webSearch 自动保存：输入防抖 600ms；与已持久化快照相同则跳过（含回填）。
  // 不在 effect 卸载时取消定时器 —— 离开页面前的最后一次编辑仍应落盘
  // （settingsState 为模块级 store，组件销毁后依然可写）。
  let webSearchSaveTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const provider = webSearchProvider;
    const apiKey = webSearchApiKey;
    const snapshot = webSearchSnapshot(provider, apiKey);
    if (snapshot === webSearchPersisted) return;
    clearTimeout(webSearchSaveTimer);
    webSearchSaveTimer = setTimeout(async () => {
      try {
        await settingsState.updateSettings({
          section: "agent",
          data: { webSearch: { provider, apiKey } },
        });
        webSearchPersisted = snapshot;
      } catch (error) {
        console.error("更新网络搜索设置失败:", error);
      }
    }, 600);
  });
</script>

<div class="p-6 pr-8 pt-2 flex flex-col gap-y-4">
  <div class="flex flex-col gap-y-1">
    <p class="text-sm text-base-content/60">
      {t("settings.agentTools.description")}
    </p>
  </div>

  <TableGroup>
    {#each codingAgentTools as tool (tool.id)}
      <SwitchRow
        label={t(tool.labelKey)}
        checked={isEnabled(tool.id)}
        onChange={(checked) => handleToggle(tool.id, checked)}
      />
    {/each}
  </TableGroup>

  <div class="flex flex-col gap-y-1 mt-2">
    <p class="text-sm font-medium text-base-content">
      {t("settings.agentTools.webSearch.title")}
    </p>
  </div>

  <TableGroup>
    <SwitchRow
      label={t("agent.tool.web_search")}
      checked={isEnabled("web_search")}
      onChange={(checked) => handleToggle("web_search", checked)}
    />
    {#if isEnabled("web_search")}
      <SelectRow
        label={t("settings.agentTools.webSearch.provider")}
        options={webSearchProviderOptions}
        bind:selectedValue={webSearchProvider}
      />
      <TextRow
        layout="vertical"
        label={t("settings.agentTools.webSearch.apiKey")}
        placeholder={t("settings.agentTools.webSearch.apiKeyPlaceholder")}
        isPassword
        bind:value={webSearchApiKey}
      />
    {/if}
  </TableGroup>

  <div class="flex flex-col gap-y-1 mt-2">
    <p class="text-sm font-medium text-base-content">
      {t("settings.agentTools.extensions.title")}
    </p>
  </div>

  <TableGroup>
    {#each extensionTools as tool (tool.id)}
      <SwitchRow
        label={tool.label}
        description={tool.desc}
        checked={isEnabled(tool.id)}
        onChange={(checked) => handleToggle(tool.id, checked)}
      />
    {/each}
  </TableGroup>
</div>
