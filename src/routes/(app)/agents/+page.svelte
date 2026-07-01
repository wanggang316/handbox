<script lang="ts">
  import { onMount } from "svelte";
  import {
    Plus,
    Bot,
    Pencil,
    Trash2,
    Play,
    Copy,
    LayoutTemplate,
  } from "@lucide/svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import { agentState, agentActions } from "$lib/states/agent.svelte";
  import { genuiState, genuiActions } from "$lib/states/genui.svelte";
  import { t } from "$lib/i18n";
  import type { Agent, GenUi } from "$lib/types";
  import ConfirmModal from "$lib/components/ui/ConfirmModal.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Tabs from "$lib/components/ui/Tabs.svelte";
  import AgentFormModal from "$lib/components/agent/AgentFormModal.svelte";
  import type { AgentFormData } from "$lib/components/agent/AgentFormModal.svelte";
  import { agentSessionActions } from "$lib/states/agentSession.svelte";

  // 当前激活的标签页：Agents / GenUI。返回链接通过 ?tab=genui 直接定位到 GenUI 列表。
  let activeTab = $state<"agents" | "genui">(
    $page.url.searchParams.get("tab") === "genui" ? "genui" : "agents"
  );

  const tabItems = [
    { value: "agents", label: "Agents" },
    { value: "genui", label: "GenUI" },
  ];

  let showFormModal = $state(false);
  let editingAgent = $state<Agent | null>(null);
  let showDeleteConfirm = $state(false);
  let selectedAgent = $state<Agent | null>(null);

  // GenUI 删除确认
  let showGenuiDeleteConfirm = $state(false);
  let selectedGenui = $state<GenUi | null>(null);

  function openCreateModal() {
    editingAgent = null;
    showFormModal = true;
  }

  function openEditModal(agent: Agent) {
    editingAgent = agent;
    showFormModal = true;
  }

  function openDeleteConfirm(agent: Agent) {
    selectedAgent = agent;
    showDeleteConfirm = true;
  }

  function closeFormModal() {
    showFormModal = false;
    editingAgent = null;
  }

  async function handleSave(data: AgentFormData) {
    // 关联的 GenUI 仅在开启生成式 UI 时有效；关闭时清空关联。
    const effectiveGenuiId =
      data.generativeUi && data.genuiId ? data.genuiId : null;

    if (editingAgent?.id) {
      // 更新现有 Agent
      await agentActions.updateAgentName(editingAgent.id, data.name);

      // Helper function to compare optional values
      const hasChanged = <T,>(a: T | undefined, b: T | undefined) =>
        a !== b && !(a === undefined && b === undefined);

      if (hasChanged(data.temperature, editingAgent.temperature)) {
        await agentActions.updateAgentField(
          editingAgent.id,
          "temperature",
          data.temperature ?? null
        );
      }
      if (hasChanged(data.maxTokens, editingAgent.maxTokens)) {
        await agentActions.updateAgentField(
          editingAgent.id,
          "maxTokens",
          data.maxTokens ?? null
        );
      }
      if (data.systemPrompt !== editingAgent.systemPrompt) {
        await agentActions.updateAgentField(
          editingAgent.id,
          "systemPrompt",
          data.systemPrompt || null
        );
      }

      // MCP 服务器变更（序列化比较，避免无意义写入）
      if (
        JSON.stringify(data.mcpServers ?? []) !==
        JSON.stringify(editingAgent.mcpServers ?? [])
      ) {
        await agentActions.updateAgentField(
          editingAgent.id,
          "mcpServers",
          data.mcpServers
        );
      }

      // 生成式 UI: 显式比较布尔值，关闭时必须发送 false（不能被假值跳过）
      if ((data.generativeUi ?? false) !== (editingAgent.generativeUi ?? false)) {
        await agentActions.updateAgentField(
          editingAgent.id,
          "generativeUi",
          data.generativeUi ?? false
        );
      }

      // 关联 GenUI: 与既有值比较，变更时下发（null 表示解除关联）
      if ((editingAgent.genuiId ?? null) !== effectiveGenuiId) {
        await agentActions.updateAgentField(
          editingAgent.id,
          "genuiId",
          effectiveGenuiId
        );
      }

      // 能力字段：后端仅支持逐字段更新，变更时下发。
      if (data.description !== (editingAgent.description ?? "")) {
        await agentActions.updateAgentField(
          editingAgent.id,
          "description",
          data.description || null
        );
      }
      if (
        JSON.stringify(data.builtinTools ?? []) !==
        JSON.stringify(editingAgent.builtinTools ?? [])
      ) {
        await agentActions.updateAgentField(
          editingAgent.id,
          "builtinTools",
          data.builtinTools
        );
      }
      if (data.workingDirMode !== (editingAgent.workingDirMode ?? "optional")) {
        await agentActions.updateAgentField(
          editingAgent.id,
          "workingDirMode",
          data.workingDirMode
        );
      }
      if (
        data.toolExecutionMode !== (editingAgent.toolExecutionMode ?? "auto")
      ) {
        await agentActions.updateAgentField(
          editingAgent.id,
          "toolExecutionMode",
          data.toolExecutionMode
        );
      }
    } else {
      // 创建新 Agent（后端 create 不接受能力字段，需创建后逐项写入）
      const newAgent = await agentActions.createAgent({
        name: data.name,
        temperature: data.temperature,
        maxTokens: data.maxTokens,
        systemPrompt: data.systemPrompt || undefined,
        reasoning: undefined,
        mcpServers: data.mcpServers,
        skills: [],
        generativeUi: data.generativeUi,
        genuiId: effectiveGenuiId ?? undefined,
      });

      // 仅对非默认能力字段做 create-then-update。
      if (newAgent.id) {
        if (data.description) {
          await agentActions.updateAgentField(
            newAgent.id,
            "description",
            data.description
          );
        }
        if (data.builtinTools.length > 0) {
          await agentActions.updateAgentField(
            newAgent.id,
            "builtinTools",
            data.builtinTools
          );
        }
        if (data.workingDirMode !== "optional") {
          await agentActions.updateAgentField(
            newAgent.id,
            "workingDirMode",
            data.workingDirMode
          );
        }
        if (data.toolExecutionMode !== "auto") {
          await agentActions.updateAgentField(
            newAgent.id,
            "toolExecutionMode",
            data.toolExecutionMode
          );
        }
      }
    }
  }

  async function handleDelete() {
    if (!selectedAgent?.id) return;
    try {
      await agentActions.deleteAgent(selectedAgent.id);
      showDeleteConfirm = false;
      selectedAgent = null;
    } catch (error) {
      console.error("Failed to delete agent:", error);
    }
  }

  async function handleUseAgent(agent: Agent) {
    if (!agent.id) return;
    try {
      // 从 AgentDefinition 实例化统一的 Agent Session（能力集 + 工作目录策略由后端裁决）
      const session = await agentSessionActions.createSessionFromDefinition(
        agent.id,
      );
      // 跳转到统一的 Agent 会话页
      goto(`/agent?id=${session.id}`);
    } catch (error) {
      console.error("Failed to create session from agent:", error);
    }
  }

  async function handleCloneAgent(agent: Agent) {
    try {
      // 内置 Agent 可被克隆为自定义 Agent；create 不接受能力字段，需逐项写入。
      const newAgent = await agentActions.createAgent({
        name: `${agent.name} 副本`,
        temperature: agent.temperature,
        topP: agent.topP,
        topK: agent.topK,
        maxTokens: agent.maxTokens,
        systemPrompt: agent.systemPrompt || undefined,
        reasoning: undefined,
        mcpServers: agent.mcpServers ? [...agent.mcpServers] : [],
        skills: [],
        generativeUi: agent.generativeUi ?? false,
        genuiId: agent.genuiId || undefined,
      });

      if (newAgent.id) {
        if (agent.builtinTools && agent.builtinTools.length > 0) {
          await agentActions.updateAgentField(newAgent.id, "builtinTools", [
            ...agent.builtinTools,
          ]);
        }
        if (agent.workingDirMode) {
          await agentActions.updateAgentField(
            newAgent.id,
            "workingDirMode",
            agent.workingDirMode
          );
        }
        if (agent.toolExecutionMode) {
          await agentActions.updateAgentField(
            newAgent.id,
            "toolExecutionMode",
            agent.toolExecutionMode
          );
        }
        if (agent.description) {
          await agentActions.updateAgentField(
            newAgent.id,
            "description",
            agent.description
          );
        }
        if (agent.providerId) {
          await agentActions.updateAgentField(
            newAgent.id,
            "providerId",
            agent.providerId
          );
        }
        if (agent.thinkingLevel) {
          await agentActions.updateAgentField(
            newAgent.id,
            "thinkingLevel",
            agent.thinkingLevel
          );
        }
      }
    } catch (error) {
      console.error("Failed to clone agent:", error);
    }
  }

  function getGenuiName(agent: Agent): string | null {
    if (!agent.genuiId) return null;
    return genuiState.genuis.find((g) => g.id === agent.genuiId)?.name ?? null;
  }

  // ── GenUI 标签页操作 ──────────────────────────────────────────────────────
  function openGenuiEditor(genui: GenUi) {
    goto(`/genui/${genui.id}`);
  }

  function openGenuiCreate() {
    goto("/genui/new");
  }

  function openGenuiDeleteConfirm(genui: GenUi) {
    selectedGenui = genui;
    showGenuiDeleteConfirm = true;
  }

  async function handleGenuiDelete() {
    if (!selectedGenui?.id) return;
    try {
      await genuiActions.deleteGenui(selectedGenui.id);
      showGenuiDeleteConfirm = false;
      selectedGenui = null;
      // 关联可能被后端清空，刷新 Agent 列表以反映最新状态
      await agentActions.loadAgents();
    } catch (error) {
      console.error("Failed to delete GenUI:", error);
    }
  }

  onMount(async () => {
    await Promise.all([
      agentActions.loadAgents(),
      genuiActions.loadGenuis().catch((e) => console.error("Failed to load GenUIs:", e)),
    ]);
  });
</script>

<div class="h-full flex flex-col">
  <div class="flex-shrink-0 px-4 pt-12 border-b border-base-300">
    <Tabs value={activeTab} items={tabItems} onChange={(v) => (activeTab = v as "agents" | "genui")} />

    {#if activeTab === "agents"}
      <div class="pb-4">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-4">
            <h1 class="text-xl font-semibold text-base-content flex items-center gap-2">
              <Bot size={24} />
              Agents
            </h1>
            <span class="text-sm text-base-content/60">
              {t("agent.manage.count", { count: agentState.agents.length })}
            </span>
          </div>
          <Button
            variant="primary"
            size="sm"
            onclick={openCreateModal}
            customClass="flex items-center gap-2"
          >
            <Plus size={16} />
            {t("agent.manage.newAgent")}
          </Button>
        </div>
      </div>
    {:else}
      <div class="pb-4">
        <div class="flex items-center justify-between mb-4">
          <div class="flex items-center gap-4">
            <h1 class="text-xl font-semibold text-base-content flex items-center gap-2">
              <LayoutTemplate size={24} />
              GenUI
            </h1>
            <span class="text-sm text-base-content/60">
              共 {genuiState.genuis.length} 个模板
            </span>
          </div>
          <Button
            variant="primary"
            size="sm"
            onclick={openGenuiCreate}
            customClass="flex items-center gap-2"
          >
            <Plus size={16} />
            新建 GenUI
          </Button>
        </div>
        <p class="text-sm text-base-content/60">
          具名、可复用的 JSON-Render UI 模板，可在 Agent 表单中关联使用。
        </p>
      </div>
    {/if}
  </div>

  <div class="flex-1 min-h-0 overflow-y-auto p-4">
    {#if activeTab === "agents"}
      {#if agentState.isLoading}
        <div class="flex items-center justify-center h-full">
          <div
            class="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin"
          ></div>
        </div>
      {:else if agentState.agents.length === 0}
        <div
          class="flex flex-col items-center justify-center h-full text-base-content/50"
        >
          <Bot size={48} class="mb-4 opacity-20" />
          <p>{t("agent.manage.empty")}</p>
          <p class="text-sm mt-2">{t("agent.manage.emptyHint")}</p>
        </div>
      {:else}
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          {#each agentState.agents as agent (agent.id)}
            <div
              class="group rounded-xl border border-[var(--hairline)] bg-[var(--bg-card)] p-4 transition-all hover:border-base-content/20 hover:shadow-sm"
            >
              <div class="flex items-start justify-between gap-3">
                <div class="flex items-center gap-3 min-w-0">
                  <div
                    class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-primary/15 text-primary"
                  >
                    <Bot size={20} />
                  </div>
                  <div class="min-w-0">
                    <div class="flex items-center gap-1.5">
                      <h3 class="truncate font-medium text-base-content">
                        {agent.name}
                      </h3>
                      {#if agent.builtin}
                        <span
                          class="shrink-0 rounded bg-base-content/10 px-1.5 py-0.5 text-[10px] font-medium text-base-content/55"
                        >
                          内置
                        </span>
                      {/if}
                    </div>
                    {#if agent.description}
                      <p class="truncate text-xs text-base-content/55">
                        {agent.description}
                      </p>
                    {/if}
                  </div>
                </div>
                <div class="flex shrink-0 items-center gap-0.5">
                  <button
                    class="rounded-md p-1.5 text-base-content/45 transition-colors hover:bg-success/10 hover:text-success"
                    onclick={() => handleUseAgent(agent)}
                    title={t("agent.manage.use")}
                  >
                    <Play size={14} />
                  </button>
                  <button
                    class="rounded-md p-1.5 text-base-content/45 transition-colors hover:bg-base-content/10 hover:text-base-content"
                    onclick={() => handleCloneAgent(agent)}
                    title="克隆"
                  >
                    <Copy size={14} />
                  </button>
                  <button
                    class="rounded-md p-1.5 text-base-content/45 transition-colors hover:bg-base-content/10 hover:text-base-content"
                    onclick={() => openEditModal(agent)}
                    title={t("common.edit")}
                  >
                    <Pencil size={14} />
                  </button>
                  {#if !agent.builtin}
                    <button
                      class="rounded-md p-1.5 text-base-content/45 transition-colors hover:bg-error/10 hover:text-error"
                      onclick={() => openDeleteConfirm(agent)}
                      title={t("common.delete")}
                    >
                      <Trash2 size={14} />
                    </button>
                  {/if}
                </div>
              </div>

              {#if agent.systemPrompt}
                <p class="mt-3 line-clamp-2 text-sm text-base-content/65">
                  {agent.systemPrompt}
                </p>
              {/if}

              {#if getGenuiName(agent)}
                <div
                  class="mt-3 inline-flex items-center gap-1 rounded-md bg-primary/10 px-2 py-0.5 text-xs text-primary"
                >
                  <LayoutTemplate size={12} />
                  {getGenuiName(agent)}
                </div>
              {/if}

              <div
                class="mt-3 border-t border-[var(--hairline)] pt-2.5 text-xs text-base-content/45"
              >
                {new Date(agent.createdAt).toLocaleDateString("zh-CN")}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {:else}
      <!-- GenUI 标签页 -->
      {#if genuiState.isLoading}
        <div class="flex items-center justify-center h-full">
          <div
            class="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin"
          ></div>
        </div>
      {:else if genuiState.genuis.length === 0}
        <div
          class="flex flex-col items-center justify-center h-full text-base-content/50"
        >
          <LayoutTemplate size={48} class="mb-4 opacity-20" />
          <p>还没有 GenUI 模板</p>
          <p class="text-sm mt-2">点击右上角「新建 GenUI」创建第一个模板</p>
        </div>
      {:else}
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {#each genuiState.genuis as genui (genui.id)}
            <button
              type="button"
              class="text-left bg-base-200 rounded-lg p-4 hover:bg-base-300 transition-colors"
              onclick={() => openGenuiEditor(genui)}
            >
              <div class="flex items-start justify-between mb-3">
                <div class="flex items-center gap-2">
                  <div
                    class="w-10 h-10 rounded-lg bg-primary/20 flex items-center justify-center text-primary"
                  >
                    <LayoutTemplate size={20} />
                  </div>
                  <div>
                    <h3 class="font-medium text-base-content">{genui.name}</h3>
                    <p class="text-xs text-base-content/60">
                      {genui.spec.length} 字符
                    </p>
                  </div>
                </div>
                <div class="flex items-center gap-1">
                  <span
                    class="p-1.5 rounded-lg hover:bg-base-100 text-base-content/60 hover:text-base-content transition-colors"
                    title={t("common.edit")}
                  >
                    <Pencil size={14} />
                  </span>
                  <span
                    role="button"
                    tabindex="0"
                    class="p-1.5 rounded-lg hover:bg-error/10 text-base-content/60 hover:text-error transition-colors"
                    title={t("common.delete")}
                    onclick={(e) => {
                      e.stopPropagation();
                      openGenuiDeleteConfirm(genui);
                    }}
                    onkeydown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        e.stopPropagation();
                        openGenuiDeleteConfirm(genui);
                      }
                    }}
                  >
                    <Trash2 size={14} />
                  </span>
                </div>
              </div>

              <div class="mt-3 pt-3 border-t border-base-300 text-xs text-base-content/50">
                {new Date(genui.updatedAt).toLocaleDateString("zh-CN")}
              </div>
            </button>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</div>

<!-- Agent 表单 Modal -->
<AgentFormModal
  open={showFormModal}
  agent={editingAgent}
  onClose={closeFormModal}
  onSave={handleSave}
/>

<!-- 删除 Agent 确认框 -->
<ConfirmModal
  title={t("agent.manage.deleteTitle")}
  message={t("agent.manage.deleteConfirm")}
  confirmText={t("common.delete")}
  confirmButtonStyle="danger"
  open={showDeleteConfirm}
  onClose={() => (showDeleteConfirm = false)}
  onConfirm={handleDelete}
/>

<!-- 删除 GenUI 确认框 -->
<ConfirmModal
  title="删除 GenUI"
  message="确认要删除这份 GenUI 吗？引用它的 Agent 将自动解除关联。此操作不可撤销。"
  confirmText={t("common.delete")}
  confirmButtonStyle="danger"
  open={showGenuiDeleteConfirm}
  onClose={() => (showGenuiDeleteConfirm = false)}
  onConfirm={handleGenuiDelete}
/>
