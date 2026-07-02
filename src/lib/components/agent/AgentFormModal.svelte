<script lang="ts">
  import { onMount } from "svelte";
  import FormModal from "../ui/FormModal.svelte";
  import Textarea from "../ui/Textarea.svelte";
  import Select from "../ui/Select.svelte";
  import Checkbox from "../ui/Checkbox.svelte";
  import Toggle from "../ui/Toggle.svelte";
  import LabeledSlider from "../ui/LabeledSlider.svelte";
  import { ChevronDown, ChevronRight } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  import type { Agent } from "$lib/types";
  import type { McpServerConfig } from "$lib/types/llm";
  import { mcpState, mcpActions } from "$lib/states/mcp.svelte";
  import { genuiState, genuiActions } from "$lib/states/genui.svelte";

  interface Props {
    open: boolean;
    agent: Agent | null;
    onClose: () => void;
    onSave: (data: AgentFormData) => Promise<void>;
  }

  export interface AgentFormData {
    name: string;
    temperature?: number;
    maxTokens?: number;
    systemPrompt: string;
    mcpServers: McpServerConfig[];
    generativeUi: boolean;
    // 关联的 GenUI id；空串表示未关联
    genuiId: string;
    // ── 能力扩展字段（P2） ──
    description: string;
    builtinTools: string[];
    workingDirMode: string;
    toolExecutionMode: string;
  }

  let { open, agent, onClose, onSave }: Props = $props();

  // ── 模型参数：会话/引擎实际消费的采样参数，扁平绘制（label + toggle + slider）。
  //    仅 temperature / maxTokens —— top_p / top_k 会话层与引擎均不消费，故不在此暴露。 ──
  type ParamKey = "temperature" | "maxTokens";
  const PARAM_META: Array<{
    key: ParamKey;
    label: string;
    min: number;
    max: number;
    step: number;
    default: number;
  }> = [
    { key: "temperature", label: "Temperature", min: 0, max: 2, step: 0.1, default: 0.7 },
    { key: "maxTokens", label: "Max Tokens", min: 256, max: 16384, step: 256, default: 4096 },
  ];

  const genuiOptions = $derived([
    { value: "", label: "未关联" },
    ...genuiState.genuis.map((g) => ({ value: g.id ?? "", label: g.name })),
  ]);

  const executionModeOptions = $derived([
    { value: "auto", label: t("agent.input.autoExecution") },
    { value: "manual", label: t("agent.input.manualExecution") },
  ]);

  // ── 能力（Capability）：内置工具 / 工作目录 / 工具执行 ──
  // coding-agent 内置工具名（与后端 builtinTools 取值对齐）。
  const BUILTIN_TOOLS = ["read", "write", "edit", "bash", "grep", "find", "ls"];
  const workingDirModeOptions = [
    { value: "required", label: "必需" },
    { value: "optional", label: "可选" },
    { value: "none", label: "无" },
  ];
  const toolExecutionModeOptions = [
    { value: "auto", label: "自动" },
    { value: "manual", label: "询问" },
  ];

  // 折叠区标题：与 .form-section-label 同观感；按钮需 flex 排布 chevron，
  // 而该类是无层级 CSS（display: block 会压过 .flex），故用等效 utilities。
  const SECTION_TOGGLE_CLASS =
    "flex items-center gap-1.5 text-left text-[11px] font-semibold uppercase tracking-wider text-base-content/45 transition-colors hover:text-base-content/70";

  // 内置 Agent：名称只读、不可删除（由后端约束）。
  const isBuiltin = $derived(agent?.builtin ?? false);

  function isToolSelected(tool: string): boolean {
    return formData.builtinTools.includes(tool);
  }
  function toggleBuiltinTool(tool: string, selected: boolean) {
    if (selected) {
      if (!formData.builtinTools.includes(tool)) {
        formData.builtinTools = [...formData.builtinTools, tool];
      }
    } else {
      formData.builtinTools = formData.builtinTools.filter((x) => x !== tool);
    }
  }

  const availableServers = $derived(
    mcpState.servers.filter(
      (s) => s.enabled && s.status === "ready" && s.enabledTools.length > 0
    )
  );

  onMount(() => {
    if (genuiState.genuis.length === 0) {
      genuiActions
        .loadGenuis()
        .catch((e) => console.error("Failed to load GenUIs:", e));
    }
    if (!mcpState.initialized) {
      mcpActions
        .loadServers()
        .catch((e) => console.error("Failed to load MCP servers:", e));
    }
  });

  let localOpen = $state(false);
  $effect(() => {
    localOpen = open;
  });

  let formData = $state<AgentFormData>({
    name: "",
    systemPrompt: "",
    mcpServers: [],
    generativeUi: false,
    genuiId: "",
    description: "",
    builtinTools: [],
    workingDirMode: "optional",
    toolExecutionMode: "auto",
  });

  let paramEnabled = $state<Record<ParamKey, boolean>>({
    temperature: false,
    maxTokens: false,
  });
  let paramValues = $state<Record<ParamKey, number>>({
    temperature: 0.7,
    maxTokens: 4096,
  });

  let paramsOpen = $state(false);
  let mcpOpen = $state(true);
  let saving = $state(false);

  function isMcpSelected(serverId: string): boolean {
    return formData.mcpServers.some((s) => s.serverId === serverId);
  }
  function mcpMode(serverId: string): "auto" | "manual" {
    return (
      formData.mcpServers.find((s) => s.serverId === serverId)?.executionMode ??
      "auto"
    );
  }
  function toggleMcp(serverId: string, selected: boolean) {
    if (selected) {
      if (!formData.mcpServers.some((s) => s.serverId === serverId)) {
        const server = mcpState.servers.find((s) => s.id === serverId);
        formData.mcpServers = [
          ...formData.mcpServers,
          {
            serverId,
            executionMode: "auto",
            enabledTools: server?.enabledTools ?? [],
          },
        ];
      }
    } else {
      formData.mcpServers = formData.mcpServers.filter(
        (s) => s.serverId !== serverId
      );
    }
  }
  function setMcpMode(serverId: string, mode: "auto" | "manual") {
    formData.mcpServers = formData.mcpServers.map((s) =>
      s.serverId === serverId ? { ...s, executionMode: mode } : s
    );
  }

  async function handleSave() {
    if (!formData.name.trim()) {
      alert(t("agent.form.nameRequired"));
      return;
    }
    formData.temperature = paramEnabled.temperature
      ? paramValues.temperature
      : undefined;
    formData.maxTokens = paramEnabled.maxTokens
      ? paramValues.maxTokens
      : undefined;

    saving = true;
    try {
      await onSave(formData);
      localOpen = false;
      onClose();
    } catch (error) {
      console.error("Failed to save agent:", error);
      alert(t("agent.form.saveFailed"));
    } finally {
      saving = false;
    }
  }

  $effect(() => {
    if (agent) {
      formData = {
        name: agent.name,
        temperature: agent.temperature,
        maxTokens: agent.maxTokens,
        systemPrompt: agent.systemPrompt || "",
        mcpServers: agent.mcpServers ? [...agent.mcpServers] : [],
        generativeUi: agent.generativeUi ?? false,
        genuiId: agent.genuiId ?? "",
        description: agent.description ?? "",
        builtinTools: agent.builtinTools ? [...agent.builtinTools] : [],
        workingDirMode: agent.workingDirMode ?? "optional",
        toolExecutionMode: agent.toolExecutionMode ?? "auto",
      };
    } else {
      formData = {
        name: "",
        systemPrompt: "",
        mcpServers: [],
        generativeUi: false,
        genuiId: "",
        description: "",
        builtinTools: [],
        workingDirMode: "optional",
        toolExecutionMode: "auto",
      };
    }

    const source: Record<ParamKey, number | undefined | null> = {
      temperature: agent?.temperature,
      maxTokens: agent?.maxTokens,
    };
    for (const p of PARAM_META) {
      const v = source[p.key];
      const has = v !== undefined && v !== null;
      paramEnabled[p.key] = has;
      paramValues[p.key] = has ? (v as number) : p.default;
    }
  });
</script>

<FormModal
  bind:open={localOpen}
  size="lg"
  title={agent ? t("agent.form.editTitle") : t("agent.form.createTitle")}
  {onClose}
  {saving}
  submitLabel={saving
    ? t("common.saving")
    : agent
      ? t("common.save")
      : t("common.create")}
  submitDisabled={saving || !formData.name.trim()}
  onSubmit={handleSave}
>
  <!-- 主区：大标题式名称 + 一行描述 + 系统提示词 -->
  <div class="flex flex-col gap-1">
    <input
      class="modal-title-input"
      bind:value={formData.name}
      placeholder={t("agent.form.namePlaceholder")}
      disabled={isBuiltin}
    />
    <input
      class="w-full bg-transparent text-sm text-base-content/80 outline-none placeholder:text-base-content/35"
      bind:value={formData.description}
      placeholder="一行简介，便于在列表中识别"
    />
  </div>

  <div class="mt-6 flex flex-col gap-2.5">
    <span class="form-section-label">{t("agent.form.systemPromptTitle")}</span>
    <Textarea
      bind:value={formData.systemPrompt}
      placeholder={t("agent.systemPrompt.placeholder")}
      rows={10}
    />
    <div class="text-right text-xs text-base-content/35">
      {t("agent.form.charCount", { count: formData.systemPrompt.length })}
    </div>
  </div>

  {#snippet aside()}
    <!-- 配置栏：能力 / 生成式 UI / 模型参数 / MCP 服务器，紧凑纵排 -->
    <div class="flex flex-col gap-6 pt-1">
      <div class="flex flex-col gap-2.5">
        <span class="form-section-label">能力</span>
        <div class="flex flex-wrap gap-x-3 gap-y-2">
          {#each BUILTIN_TOOLS as tool (tool)}
            <Checkbox
              checked={isToolSelected(tool)}
              onCheckedChange={(v) => toggleBuiltinTool(tool, v)}
            >
              <span class="font-mono text-base-content/85">{tool}</span>
            </Checkbox>
          {/each}
        </div>
        <div class="flex flex-col gap-1">
          <span class="text-xs text-base-content/70">工作目录</span>
          <Select
            options={workingDirModeOptions}
            bind:selectedValue={formData.workingDirMode}
            size="sm"
          />
        </div>
        <div class="flex flex-col gap-1">
          <span class="text-xs text-base-content/70">工具执行</span>
          <Select
            options={toolExecutionModeOptions}
            bind:selectedValue={formData.toolExecutionMode}
            size="sm"
          />
        </div>
      </div>

      <div class="flex flex-col gap-2.5">
        <div class="flex items-start justify-between gap-3">
          <div class="flex flex-col gap-0.5">
            <span class="form-section-label">生成式 UI</span>
            <span class="text-xs text-base-content/50">
              允许助手在回复中渲染交互式界面
            </span>
          </div>
          <Toggle bind:checked={formData.generativeUi} />
        </div>
        {#if formData.generativeUi}
          <div class="flex flex-col gap-1">
            <Select
              options={genuiOptions}
              bind:selectedValue={formData.genuiId}
              size="sm"
            />
            <span class="text-xs text-base-content/50">选择一份已保存的模板</span>
          </div>
        {/if}
      </div>

      <div class="flex flex-col gap-3">
        <button
          type="button"
          class={SECTION_TOGGLE_CLASS}
          onclick={() => (paramsOpen = !paramsOpen)}
        >
          {#if paramsOpen}
            <ChevronDown size={13} />
          {:else}
            <ChevronRight size={13} />
          {/if}
          {t("agent.form.modelParams")}
        </button>
        {#if paramsOpen}
          <div class="flex flex-col gap-3">
            {#each PARAM_META as p (p.key)}
              <div class="flex flex-col gap-2">
                <div class="flex items-center justify-between">
                  <span class="text-sm text-base-content/85">{p.label}</span>
                  <Toggle bind:checked={paramEnabled[p.key]} />
                </div>
                {#if paramEnabled[p.key]}
                  <LabeledSlider
                    bind:value={paramValues[p.key]}
                    min={p.min}
                    max={p.max}
                    step={p.step}
                    showValue={true}
                  />
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <div class="flex flex-col gap-3">
        <button
          type="button"
          class={SECTION_TOGGLE_CLASS}
          onclick={() => (mcpOpen = !mcpOpen)}
        >
          {#if mcpOpen}
            <ChevronDown size={13} />
          {:else}
            <ChevronRight size={13} />
          {/if}
          {t("agent.form.mcpServers")}
        </button>
        {#if mcpOpen}
          {#if availableServers.length === 0}
            <div class="rounded-md border border-dashed border-[var(--hairline)] px-3 py-4 text-center">
              <p class="text-sm text-base-content/55">
                {t("agent.input.noAvailableMcpServers")}
              </p>
              <p class="mt-0.5 text-xs text-base-content/40">
                {t("agent.input.configureMcpInSettings")}
              </p>
            </div>
          {:else}
            <div class="flex flex-col gap-3">
              {#each availableServers as server (server.id)}
                <div class="flex flex-col gap-1">
                  <div class="flex items-center justify-between gap-2">
                    <span class="truncate text-sm text-base-content/85">
                      {server.displayName ?? server.name}
                    </span>
                    <Toggle
                      checked={isMcpSelected(server.id)}
                      onChange={(v) => toggleMcp(server.id, v)}
                    />
                  </div>
                  <div class="flex items-center justify-between gap-2">
                    <span class="text-xs text-base-content/40">
                      {t("agent.input.enabledToolsCount", {
                        count: server.enabledTools.length,
                      })}
                    </span>
                    {#if isMcpSelected(server.id)}
                      <Select
                        options={executionModeOptions}
                        selectedValue={mcpMode(server.id)}
                        onSelect={(value) =>
                          setMcpMode(server.id, value as "auto" | "manual")}
                        size="sm"
                        autoWidth={true}
                      />
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        {/if}
      </div>
    </div>
  {/snippet}
</FormModal>
