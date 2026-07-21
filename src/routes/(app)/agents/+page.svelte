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
  import PageHeader from "$lib/components/ui/PageHeader.svelte";
  import { page } from "$app/stores";
  import { agentState, agentActions } from "$lib/states/agent.svelte";
  import { genuiState, genuiActions } from "$lib/states/genui.svelte";
  import { resolveAgentIcon } from "$lib/utils/agentIcons";
  import { t } from "$lib/i18n";
  import type { Agent, GenUi } from "$lib/types";
  import ConfirmModal from "$lib/components/ui/ConfirmModal.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Spinner from "$lib/components/ui/Spinner.svelte";
  import Tabs from "$lib/components/ui/Tabs.svelte";
  import { agentSessionActions } from "$lib/states/agentSession.svelte";

  // 当前激活的标签页：Agents / GenUI。返回链接通过 ?tab=genui 直接定位到 GenUI 列表。
  let activeTab = $state<"agents" | "genui">(
    $page.url.searchParams.get("tab") === "genui" ? "genui" : "agents"
  );

  const tabItems = [
    { value: "agents", label: "Agents" },
    { value: "genui", label: "GenUI" },
  ];

  let showDeleteConfirm = $state(false);
  let selectedAgent = $state<Agent | null>(null);

  // GenUI 删除确认
  let showGenuiDeleteConfirm = $state(false);
  let selectedGenui = $state<GenUi | null>(null);

  // 创建 / 编辑走二级页（不再使用 Modal）。
  function openCreate() {
    goto("/agents/new");
  }

  function openEdit(agent: Agent) {
    if (!agent.id) return;
    goto(`/agents/${agent.id}`);
  }

  function openDeleteConfirm(agent: Agent) {
    selectedAgent = agent;
    showDeleteConfirm = true;
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
        skills: agent.skills ? [...agent.skills] : [],
        generativeUi: agent.generativeUi ?? false,
        genuiId: agent.genuiId || undefined,
      });

      if (newAgent.id) {
        if (agent.icon) {
          await agentActions.updateAgentField(newAgent.id, "icon", agent.icon);
        }
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

<div class="h-full flex flex-col bg-[var(--bg-canvas)]">
  <div class="flex-shrink-0 px-6 pb-1 pt-12">
    <div class="mx-auto w-full max-w-3xl">
    <Tabs value={activeTab} items={tabItems} onChange={(v) => (activeTab = v as "agents" | "genui")} />

    </div>
  </div>

  <div class="flex-1 min-h-0 overflow-y-auto px-6 pb-6">
    <div class="mx-auto w-full max-w-3xl">
      <!-- 页头随内容滚动（Codex 式），顶部只固定 Tabs -->
      {#if activeTab === "agents"}
        <div class="pb-5 pt-6">
          <PageHeader
            title="Agents"
            meta={t("agent.manage.count", { count: agentState.agents.length })}
          >
            {#snippet actions()}
              <Button
                variant="primary"
                size="sm"
                onclick={openCreate}
                customClass="flex items-center gap-2"
              >
                <Plus size={16} />
                {t("agent.manage.newAgent")}
              </Button>
            {/snippet}
          </PageHeader>
        </div>
      {:else}
        <div class="pb-5 pt-6">
          <PageHeader
            title="GenUI"
            meta={`共 ${genuiState.genuis.length} 个模板`}
          >
            {#snippet actions()}
              <Button
                variant="primary"
                size="sm"
                onclick={openGenuiCreate}
                customClass="flex items-center gap-2"
              >
                <Plus size={16} />
                新建 GenUI
              </Button>
            {/snippet}
          </PageHeader>
        </div>
      {/if}
    {#if activeTab === "agents"}
      <!-- Spinner 仅在冷启动（列表为空）时顶替内容；已有缓存则立即渲染、后台刷新
           不 blank，避免每次导航都闪一下 spinner（感知为切换延迟）。 -->
      {#if agentState.isLoading && agentState.agents.length === 0}
        <div class="flex items-center justify-center h-full">
          <Spinner size={28} />
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
        <!-- 列表：一行一个 Agent（图标 + 名称/描述 + 徽标 + 日期 + hover 操作） -->
        <div
          class="flex flex-col divide-y divide-[var(--hairline)] overflow-hidden rounded-xl border border-[var(--hairline)] bg-[var(--bg-panel)]"
        >
          {#each agentState.agents as agent (agent.id)}
            {@const AgentIcon = resolveAgentIcon(agent.icon)}
            <div
              class="group flex items-center gap-3 px-4 py-3 transition-colors hover:bg-base-300/40"
            >
              <div
                class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-base-200 text-base-content/60"
              >
                <AgentIcon size={16} />
              </div>
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-1.5">
                  <h3 class="truncate text-sm font-medium text-base-content">
                    {agent.name}
                  </h3>
                  {#if agent.builtin}
                    <span
                      class="shrink-0 rounded bg-base-content/10 px-1.5 py-0.5 text-[10px] font-medium text-base-content/55"
                    >
                      内置
                    </span>
                  {/if}
                  {#if getGenuiName(agent)}
                    <span
                      class="inline-flex shrink-0 items-center gap-1 rounded-md bg-base-200 px-1.5 py-0.5 text-[10px] text-base-content/60"
                    >
                      <LayoutTemplate size={10} />
                      {getGenuiName(agent)}
                    </span>
                  {/if}
                </div>
              </div>
              <!-- 操作：hover / 键盘聚焦时显现 -->
              <div
                class="flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity focus-within:opacity-100 group-hover:opacity-100"
              >
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
                  onclick={() => openEdit(agent)}
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
          {/each}
        </div>
      {/if}
    {:else}
      <!-- GenUI 标签页 -->
      {#if genuiState.isLoading}
        <div class="flex items-center justify-center h-full">
          <Spinner size={28} />
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
        <!-- 列表：一行一个 GenUI（图标 + 名称 + hover 操作），与 Agents 列表一致 -->
        <div
          class="flex flex-col divide-y divide-[var(--hairline)] overflow-hidden rounded-xl border border-[var(--hairline)] bg-[var(--bg-panel)]"
        >
          {#each genuiState.genuis as genui (genui.id)}
            <div
              class="group flex items-center gap-3 px-4 py-3 transition-colors hover:bg-base-300/40"
            >
              <div
                class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-base-200 text-base-content/60"
              >
                <LayoutTemplate size={16} />
              </div>
              <div class="min-w-0 flex-1">
                <h3 class="truncate text-sm font-medium text-base-content">
                  {genui.name}
                </h3>
              </div>
              <!-- 操作：hover / 键盘聚焦时显现 -->
              <div
                class="flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity focus-within:opacity-100 group-hover:opacity-100"
              >
                <button
                  class="rounded-md p-1.5 text-base-content/45 transition-colors hover:bg-base-content/10 hover:text-base-content"
                  onclick={() => openGenuiEditor(genui)}
                  title={t("common.edit")}
                >
                  <Pencil size={14} />
                </button>
                <button
                  class="rounded-md p-1.5 text-base-content/45 transition-colors hover:bg-error/10 hover:text-error"
                  onclick={() => openGenuiDeleteConfirm(genui)}
                  title={t("common.delete")}
                >
                  <Trash2 size={14} />
                </button>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
    </div>
  </div>
</div>

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
