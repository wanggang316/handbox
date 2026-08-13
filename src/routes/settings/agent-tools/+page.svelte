<script lang="ts">
  import { onMount } from "svelte";
  import { TableGroup, SwitchRow, SelectRow, TextRow } from "$lib/components/ui/table";
  import { settingsState } from "$lib/states";
  import { BUILTIN_TOOLS, BUILTIN_TOOL_IDS } from "$lib/constants/agentTools";
  import { t } from "$lib/i18n";

  // Globally enabled default tool set (coding-agent registry names); a missing
  // agent section means all enabled.
  let enabledTools = $state<string[]>([...BUILTIN_TOOL_IDS]);

  let webSearchProvider = $state("tavily");
  let webSearchApiKey = $state("");
  // Last persisted (or backfilled) webSearch snapshot — the debounced $effect
  // uses it to skip backfill-induced pseudo-changes and write only on real edits.
  let webSearchPersisted = $state("");

  const webSearchProviderOptions = [{ value: "tavily", label: "Tavily" }];

  // Grouping: the first group lists only coding-agent builtins; web_search sits
  // with its provider config; render_card / render_app / ask_question form the
  // UI-extension group (HandBox-native surfaces); skill stands alone. Every
  // extension id must be listed here, or it falls through into the builtin
  // group and is mislabelled as a coding-agent tool.
  const EXTENSION_IDS = [
    "web_search",
    "render_card",
    "render_app",
    "ask_question",
    "skill",
  ];
  const codingAgentTools = BUILTIN_TOOLS.filter(
    (tool) => !EXTENSION_IDS.includes(tool.id),
  );
  const uiExtensionTools = $derived([
    { id: "render_card", label: t("agent.tool.render_card"), desc: t("settings.agentTools.renderCardDesc") },
    { id: "render_app", label: t("agent.tool.render_app"), desc: t("settings.agentTools.renderAppDesc") },
    { id: "ask_question", label: t("agent.tool.ask_question"), desc: t("settings.agentTools.askQuestionDesc") },
  ]);

  function webSearchSnapshot(provider: string, apiKey: string): string {
    return JSON.stringify({ provider, apiKey });
  }

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

  // Root layout preloaded settings: sync backfill so the first frame shows real
  // values (no toggle flicker).
  syncFromSettings();

  // Cold-start/deep-link fallback: resync once settings finish loading
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

  // webSearch autosave: 600ms debounce; skip when equal to the persisted
  // snapshot (covers backfill). The timer is deliberately not cancelled on
  // effect teardown — the last edit before leaving must still persist
  // (settingsState is a module-level store, writable after unmount).
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

  <div class="flex flex-col gap-y-1">
    <p class="text-sm font-medium text-base-content">
      {t("settings.agentTools.system.title")}
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
      {t("settings.agentTools.uiExtensions.title")}
    </p>
  </div>

  <TableGroup>
    {#each uiExtensionTools as tool (tool.id)}
      <SwitchRow
        label={tool.label}
        description={tool.desc}
        checked={isEnabled(tool.id)}
        onChange={(checked) => handleToggle(tool.id, checked)}
      />
    {/each}
  </TableGroup>

  <div class="flex flex-col gap-y-1 mt-2">
    <p class="text-sm font-medium text-base-content">
      {t("settings.agentTools.skill.title")}
    </p>
  </div>

  <TableGroup>
    <SwitchRow
      label={t("agent.tool.skill")}
      description={t("settings.agentTools.skillDesc")}
      checked={isEnabled("skill")}
      onChange={(checked) => handleToggle("skill", checked)}
    />
  </TableGroup>
</div>
