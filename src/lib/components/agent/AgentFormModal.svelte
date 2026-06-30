<script lang="ts">
  import { onMount } from "svelte";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import Select from "../ui/Select.svelte";
  import Toggle from "../ui/Toggle.svelte";
  import LabeledSlider from "../ui/LabeledSlider.svelte";
  import ChatModelSelectModal from "../chat/ChatModelSelectModal.svelte";
  import { ChevronsUpDown, ChevronDown, ChevronRight } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  import type { Agent } from "$lib/types";
  import type { McpServerConfig } from "$lib/types/chat";
  import type { ModelWithProvider } from "$lib/types/provider";
  import {
    getAllModels,
    getProviderIconById,
    providerActions,
    providerState,
  } from "$lib/states/provider.svelte";
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
    model: string;
    temperature?: number;
    topP?: number;
    topK?: number;
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

  // 统一控件外观（Linear 阶梯）：控件底色上浮一阶到 surface-3，与 Modal 卡片
  // (--bg-card) 拉开层次；hairline 细边框，hover 加深。扁平排布，不再套内层卡片。
  const FIELD_CLASS =
    "w-full rounded-md bg-[var(--surface-3)] border border-[var(--hairline)] px-2.5 py-2 " +
    "text-sm text-base-content placeholder:text-base-content/35 transition-colors " +
    "hover:border-[var(--hairline-strong)]";
  const LABEL_CLASS = "text-[13px] font-medium text-base-content/70";
  const SECTION_CLASS =
    "text-[11px] font-semibold uppercase tracking-wider text-base-content/40";

  // ── 模型参数：与聊天侧同一组字段，扁平绘制（label + toggle + slider）。 ──
  type ParamKey = "temperature" | "topP" | "topK" | "maxTokens";
  const PARAM_META: Array<{
    key: ParamKey;
    label: string;
    min: number;
    max: number;
    step: number;
    default: number;
  }> = [
    { key: "temperature", label: "Temperature", min: 0, max: 2, step: 0.1, default: 0.7 },
    { key: "topP", label: "Top P", min: 0, max: 1, step: 0.05, default: 0.9 },
    { key: "topK", label: "Top K", min: 0, max: 100, step: 1, default: 40 },
    { key: "maxTokens", label: "Max Tokens", min: 256, max: 16384, step: 256, default: 4096 },
  ];

  const genuiOptions = $derived([
    { value: "", label: "未关联" },
    ...genuiState.genuis.map((g) => ({ value: g.id ?? "", label: g.name })),
  ]);

  const executionModeOptions = $derived([
    { value: "auto", label: t("chat.autoExecution") },
    { value: "manual", label: t("chat.manualExecution") },
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
    if (providerState.providersWithModels.length === 0) {
      providerActions
        .loadProvidersWithModels()
        .catch((e) => console.error("Failed to load models:", e));
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
    model: "",
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
    topP: false,
    topK: false,
    maxTokens: false,
  });
  let paramValues = $state<Record<ParamKey, number>>({
    temperature: 0.7,
    topP: 0.9,
    topK: 40,
    maxTokens: 4096,
  });

  let paramsOpen = $state(false);
  let mcpOpen = $state(true);
  let showModelModal = $state(false);
  let saving = $state(false);

  const selectedModel = $derived<ModelWithProvider | null>(
    formData.model
      ? getAllModels().find((m) => m.id === formData.model) ?? null
      : null
  );
  const providerIcon = $derived(
    selectedModel?.provider_id
      ? getProviderIconById(selectedModel.provider_id)
      : undefined
  );

  function handleModelSelect(model: ModelWithProvider) {
    formData.model = model.id;
    showModelModal = false;
  }

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
    formData.topP = paramEnabled.topP ? paramValues.topP : undefined;
    formData.topK = paramEnabled.topK ? paramValues.topK : undefined;
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
        model: agent.model || "",
        temperature: agent.temperature,
        topP: agent.topP,
        topK: agent.topK,
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
        model: "",
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
      topP: agent?.topP,
      topK: agent?.topK,
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

<Modal
  bind:open={localOpen}
  title={agent ? t("agent.form.editTitle") : t("agent.form.createTitle")}
  {onClose}
>
  <div
    class="flex w-[460px] max-h-[82vh] flex-col gap-5 overflow-y-auto px-6 pt-14 pb-0"
  >
    <!-- 基本字段：扁平 label + 控件 -->
    <div class="flex flex-col gap-3.5">
      <div class="flex flex-col gap-1.5">
        <span class={LABEL_CLASS}>{t("agent.form.nameLabel")}</span>
        <input
          class="{FIELD_CLASS} disabled:cursor-not-allowed disabled:opacity-60"
          placeholder={t("agent.form.namePlaceholder")}
          bind:value={formData.name}
          disabled={isBuiltin}
        />
      </div>

      <div class="flex flex-col gap-1.5">
        <span class={LABEL_CLASS}>描述</span>
        <input
          class={FIELD_CLASS}
          placeholder="一行简介，便于在列表中识别"
          bind:value={formData.description}
        />
      </div>

      <div class="flex flex-col gap-1.5">
        <span class={LABEL_CLASS}>{t("agent.form.modelLabel")}</span>
        <button
          type="button"
          class="{FIELD_CLASS} text-left"
          onclick={() => (showModelModal = true)}
        >
          {#if selectedModel}
            <div class="flex items-center justify-between gap-2">
              <div class="flex min-w-0 items-center gap-2">
                {#if providerIcon}
                  <img
                    src={providerIcon}
                    alt={selectedModel.providerName}
                    class="h-4 w-4 shrink-0 rounded object-contain"
                  />
                {/if}
                <span class="truncate text-base-content">{selectedModel.name}</span>
                <span class="shrink-0 text-xs text-base-content/40">
                  {selectedModel.providerName}
                </span>
              </div>
              <ChevronsUpDown size={14} class="shrink-0 text-base-content/40" />
            </div>
          {:else if formData.model}
            <div class="flex items-center justify-between gap-2">
              <span class="truncate font-mono text-base-content/80">
                {formData.model}
              </span>
              <ChevronsUpDown size={14} class="shrink-0 text-base-content/40" />
            </div>
          {:else}
            <div class="flex items-center justify-between gap-2">
              <span class="text-base-content/40">{t("chat.selectModel")}</span>
              <ChevronsUpDown size={14} class="shrink-0 text-base-content/40" />
            </div>
          {/if}
        </button>
      </div>

      <div class="flex flex-col gap-1.5">
        <span class={LABEL_CLASS}>{t("agent.form.systemPromptTitle")}</span>
        <textarea
          bind:value={formData.systemPrompt}
          placeholder={t("agent.systemPrompt.placeholder")}
          rows="4"
          class="{FIELD_CLASS} resize-none font-mono leading-relaxed"
        ></textarea>
        <div class="text-right text-xs text-base-content/35">
          {t("agent.form.charCount", { count: formData.systemPrompt.length })}
        </div>
      </div>
    </div>

    <!-- 生成式 UI -->
    <div class="flex flex-col gap-3">
      <div class="flex items-center justify-between gap-3">
        <div class="flex flex-col gap-0.5">
          <span class={LABEL_CLASS}>生成式 UI</span>
          <span class="text-xs text-base-content/45">
            允许助手在回复中渲染交互式界面
          </span>
        </div>
        <Toggle bind:checked={formData.generativeUi} />
      </div>
      {#if formData.generativeUi}
        <div class="flex flex-col gap-1.5">
          <Select options={genuiOptions} bind:selectedValue={formData.genuiId} />
          <p class="text-xs text-base-content/40">
            选择一份已保存的 GenUI 模板（可在 Agents 页的 GenUI 标签中创建与管理）。
          </p>
        </div>
      {/if}
    </div>

    <!-- 能力（Capability）：内置工具 / 工作目录 / 工具执行 -->
    <div class="flex flex-col gap-3.5">
      <span class={SECTION_CLASS}>能力</span>

      <div class="flex flex-col gap-1.5">
        <span class={LABEL_CLASS}>内置工具</span>
        <div class="flex flex-wrap gap-x-4 gap-y-2">
          {#each BUILTIN_TOOLS as tool (tool)}
            <label class="flex cursor-pointer items-center gap-2">
              <input
                type="checkbox"
                class="h-3.5 w-3.5 accent-primary"
                checked={isToolSelected(tool)}
                onchange={(e) =>
                  toggleBuiltinTool(tool, e.currentTarget.checked)}
              />
              <span class="font-mono text-sm text-base-content/85">{tool}</span>
            </label>
          {/each}
        </div>
      </div>

      <div class="flex flex-col gap-1.5">
        <span class={LABEL_CLASS}>工作目录</span>
        <Select
          options={workingDirModeOptions}
          bind:selectedValue={formData.workingDirMode}
        />
      </div>

      <div class="flex flex-col gap-1.5">
        <span class={LABEL_CLASS}>工具执行</span>
        <Select
          options={toolExecutionModeOptions}
          bind:selectedValue={formData.toolExecutionMode}
        />
      </div>
    </div>

    <!-- 模型参数：可折叠分组，扁平 slider 行 -->
    <div class="flex flex-col gap-3">
      <button
        type="button"
        class="flex items-center gap-1.5 text-left {SECTION_CLASS} hover:text-base-content/60"
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

    <!-- MCP 服务器：可折叠分组 -->
    <div class="flex flex-col gap-3">
      <button
        type="button"
        class="flex items-center gap-1.5 text-left {SECTION_CLASS} hover:text-base-content/60"
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
              {t("chat.noAvailableMcpServers")}
            </p>
            <p class="mt-0.5 text-xs text-base-content/40">
              {t("chat.configureMcpInSettings")}
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
                    {t("chat.enabledToolsCount", {
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

    <!-- 底部按钮：sticky 底栏，内容超长时始终可见 -->
    <div
      class="sticky bottom-0 -mx-6 mt-1 flex items-center justify-end gap-3 border-t border-[var(--hairline)] bg-[var(--bg-card)] px-6 pb-5 pt-4"
    >
      <Button variant="ghost" onclick={onClose} disabled={saving}>
        {t("common.cancel")}
      </Button>
      <Button
        variant="primary"
        onclick={handleSave}
        disabled={saving || !formData.name.trim()}
      >
        {saving ? t("common.saving") : agent ? t("common.save") : t("common.create")}
      </Button>
    </div>
  </div>
</Modal>

<!-- 模型选择 Modal（叠在表单之上） -->
<ChatModelSelectModal
  bind:open={showModelModal}
  selectedModel={selectedModel ?? null}
  onModelSelect={handleModelSelect}
/>
