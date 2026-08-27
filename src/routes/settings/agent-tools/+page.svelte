<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import {
    TableGroup,
    TableBaseRow,
    SwitchRow,
    SelectRow,
    TextRow,
  } from "$lib/components/ui/table";
  import Button from "$lib/components/ui/Button.svelte";
  import { Plus } from "@lucide/svelte";
  import { settingsState } from "$lib/states";
  import {
    toolDefinitionState,
    toolDefinitionActions,
  } from "$lib/states/toolDefinition.svelte";
  import { resolveAgentIcon } from "$lib/utils/agentIcons";
  import {
    BUILTIN_TOOLS,
    BUILTIN_TOOL_IDS,
    resolveToolIcon,
  } from "$lib/constants/agentTools";
  import { t } from "$lib/i18n";
  import type { ToolDefinition } from "$lib/types/toolDefinition";

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

  // Same glyphs the timeline puts on a tool call, so a row here and a call
  // there are recognisably the same tool.
  const webSearchIcon = resolveToolIcon("web_search");
  const skillIcon = resolveToolIcon("skill");

  /**
   * Detail view for any tool. One route serves all three sources — a custom
   * tool's uuid, a built-in id, an `mcp__…` name — because none of those can
   * collide; the page decides editable vs read-only from what the id resolves
   * to. `skill` has no tool of its own to show, so its row has no detail.
   */
  function openToolDetail(toolId: string): void {
    void goto(`/settings/agent-tools/${encodeURIComponent(toolId)}`);
  }

  /** Enable/disable is the definition's own flag; it gates every session. */
  async function toggleCustomTool(
    tool: ToolDefinition,
    enabled: boolean,
  ): Promise<void> {
    try {
      await toolDefinitionActions.updateTool(tool.id, { enabled });
    } catch (error) {
      console.error("更新自定义工具失败:", error);
    }
  }

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
    // Always re-read: a tool may have been created, renamed or deleted on the
    // detail page since the store last loaded.
    toolDefinitionActions.loadTools().catch((error) => {
      console.error("加载自定义工具失败:", error);
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

  <div class="flex flex-col gap-y-1 mt-2">
    <p class="text-sm font-medium text-base-content">
      {t("settings.agentTools.system.title")}
    </p>
  </div>

  <TableGroup>
    {#each codingAgentTools as tool (tool.id)}
      <SwitchRow
        label={t(tool.labelKey)}
        icon={tool.icon}
        checked={isEnabled(tool.id)}
        onChange={(checked) => handleToggle(tool.id, checked)}
        onOpenDetail={() => openToolDetail(tool.id)}
        detailAriaLabel={t(tool.labelKey)}
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
      icon={webSearchIcon}
      checked={isEnabled("web_search")}
      onChange={(checked) => handleToggle("web_search", checked)}
      onOpenDetail={() => openToolDetail("web_search")}
      detailAriaLabel={t("agent.tool.web_search")}
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
        icon={resolveToolIcon(tool.id)}
        description={tool.desc}
        checked={isEnabled(tool.id)}
        onChange={(checked) => handleToggle(tool.id, checked)}
        onOpenDetail={() => openToolDetail(tool.id)}
        detailAriaLabel={tool.label}
      />
    {/each}
  </TableGroup>

  <div class="flex flex-col gap-y-1 mt-2">
    <p class="text-sm font-medium text-base-content">
      {t("settings.agentTools.skill.title")}
    </p>
  </div>

  <TableGroup>
    <!-- No detail: `skill` gates the skill pipeline rather than registering a
         tool, so there is no description or schema to show. -->
    <SwitchRow
      label={t("agent.tool.skill")}
      icon={skillIcon}
      description={t("settings.agentTools.skillDesc")}
      checked={isEnabled("skill")}
      onChange={(checked) => handleToggle("skill", checked)}
    />
  </TableGroup>

  <div class="flex flex-col gap-y-1 mt-2">
    <div class="flex items-start justify-between gap-4">
      <div class="flex flex-col gap-y-1">
        <p class="text-sm font-medium text-base-content">
          {t("settings.tools.custom.title")}
        </p>
        <p class="text-[13px] leading-snug text-base-content/55">
          {t("settings.tools.custom.description")}
        </p>
      </div>
      <Button
        variant="secondary"
        size="sm"
        class="shrink-0"
        onclick={() => openToolDetail("new")}
      >
        <Plus size={14} />
        {t("settings.tools.custom.new")}
      </Button>
    </div>
  </div>

  <TableGroup>
    {#if toolDefinitionState.tools.length === 0}
      <TableBaseRow>
        <p class="text-[13px] text-base-content/55">
          {t("settings.tools.custom.empty")}
        </p>
      </TableBaseRow>
    {:else}
      {#each toolDefinitionState.tools as tool (tool.id)}
        <!-- The registration name reads as the subtitle: it is what the model
             calls and what a transcript's tool row is keyed by. -->
        <SwitchRow
          label={tool.displayName}
          icon={resolveAgentIcon(tool.icon)}
          description={tool.genuiId
            ? tool.name
            : `${tool.name} · ${t("settings.tools.custom.noView")}`}
          checked={tool.enabled}
          onChange={(checked) => toggleCustomTool(tool, checked)}
          onOpenDetail={() => openToolDetail(tool.id)}
          detailAriaLabel={tool.displayName}
        />
      {/each}
    {/if}
  </TableGroup>
</div>
