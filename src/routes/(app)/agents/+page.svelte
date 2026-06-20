<script lang="ts">
  import { onMount } from "svelte";
  import { Plus, Bot, Pencil, Trash2, Settings, Play, LayoutTemplate } from "@lucide/svelte";
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
  import { createSessionFromAgent } from "$lib/api/chat";

  // 当前激活的标签页：Agents / GenUI。返回链接通过 ?tab=genui 直接定位到 GenUI 列表。
  let activeTab = $state<"agents" | "genui">(
    $page.url.searchParams.get("tab") === "genui" ? "genui" : "agents"
  );

  const tabItems = [
    { value: "agents", label: "Agents" },
    { value: "genui", label: "GenUI" },
  ];

  let searchQuery = $state("");

  let showFormModal = $state(false);
  let editingAgent = $state<Agent | null>(null);
  let showDeleteConfirm = $state(false);
  let selectedAgent = $state<Agent | null>(null);

  // GenUI 删除确认
  let showGenuiDeleteConfirm = $state(false);
  let selectedGenui = $state<GenUi | null>(null);

  const filteredAgents = $derived.by(() => {
    if (!searchQuery) return agentState.agents;
    const query = searchQuery.toLowerCase();
    return agentState.agents.filter(
      (a) =>
        a.name.toLowerCase().includes(query) ||
        a.skills.some((s) => s.toLowerCase().includes(query))
    );
  });

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

      if (data.model !== editingAgent.model) {
        await agentActions.updateAgentField(editingAgent.id, "model", data.model);
      }

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
      if (hasChanged(data.topP, editingAgent.topP)) {
        await agentActions.updateAgentField(
          editingAgent.id,
          "topP",
          data.topP ?? null
        );
      }
      if (hasChanged(data.topK, editingAgent.topK)) {
        await agentActions.updateAgentField(
          editingAgent.id,
          "topK",
          data.topK ?? null
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

      const skills = data.skills
        ? data.skills.split(",").map((s) => s.trim()).filter(Boolean)
        : [];
      if (JSON.stringify(skills) !== JSON.stringify(editingAgent.skills)) {
        await agentActions.updateAgentField(editingAgent.id, "skills", skills);
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
    } else {
      // 创建新 Agent
      await agentActions.createAgent({
        name: data.name,
        model: data.model || undefined,
        temperature: data.temperature,
        topP: data.topP,
        topK: data.topK,
        maxTokens: data.maxTokens,
        systemPrompt: data.systemPrompt || undefined,
        reasoning: undefined,
        mcpServers: [],
        skills: data.skills
          ? data.skills.split(",").map((s) => s.trim()).filter(Boolean)
          : [],
        generativeUi: data.generativeUi,
        genuiId: effectiveGenuiId ?? undefined,
      });
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
      // 通过 Agent 创建 Session
      const session = await createSessionFromAgent(agent.id);
      // 跳转到聊天页面
      goto(`/chat/${session.id}`);
    } catch (error) {
      console.error("Failed to create session from agent:", error);
    }
  }

  function getModelName(agent: Agent): string {
    return agent.model || t("agent.manage.modelUnset");
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
        <div class="flex items-center justify-between mb-4">
          <div class="flex items-center gap-4">
            <h1 class="text-xl font-semibold text-base-content flex items-center gap-2">
              <Bot size={24} />
              Agents
            </h1>
            <span class="text-sm text-base-content/60">
              {t("agent.manage.count", { count: filteredAgents.length })}
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

        <div class="relative">
          <input
            type="text"
            placeholder={t("agent.manage.searchPlaceholder")}
            class="w-full h-9 pl-10 pr-4 bg-base-200 rounded-lg text-base-content placeholder:text-base-content/50 text-sm"
            bind:value={searchQuery}
          />
          <Settings
            class="absolute left-3 top-1/2 -translate-y-1/2 text-base-content/50"
            size={16}
          />
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
      {:else if filteredAgents.length === 0}
        <div
          class="flex flex-col items-center justify-center h-full text-base-content/50"
        >
          <Bot size={48} class="mb-4 opacity-20" />
          {#if searchQuery}
            <p class="mb-2">{t("agent.manage.noMatch")}</p>
            <button
              class="text-primary hover:underline cursor-pointer"
              onclick={() => (searchQuery = "")}
            >
              {t("agent.manage.clearSearch")}
            </button>
          {:else}
            <p>{t("agent.manage.empty")}</p>
            <p class="text-sm mt-2">{t("agent.manage.emptyHint")}</p>
          {/if}
        </div>
      {:else}
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {#each filteredAgents as agent (agent.id)}
            <div
              class="bg-base-200 rounded-lg p-4 hover:bg-base-300 transition-colors"
            >
              <div class="flex items-start justify-between mb-3">
                <div class="flex items-center gap-2">
                  <div
                    class="w-10 h-10 rounded-lg bg-primary/20 flex items-center justify-center text-primary"
                  >
                    <Bot size={20} />
                  </div>
                  <div>
                    <h3 class="font-medium text-base-content">{agent.name}</h3>
                    <p class="text-xs text-base-content/60">
                      {getModelName(agent)}
                    </p>
                  </div>
                </div>
                <div class="flex items-center gap-1">
                  <button
                    class="p-1.5 rounded-lg hover:bg-success/10 text-base-content/60 hover:text-success transition-colors"
                    onclick={() => handleUseAgent(agent)}
                    title={t("agent.manage.use")}
                  >
                    <Play size={14} />
                  </button>
                  <button
                    class="p-1.5 rounded-lg hover:bg-base-100 text-base-content/60 hover:text-base-content transition-colors"
                    onclick={() => openEditModal(agent)}
                    title={t("common.edit")}
                  >
                    <Pencil size={14} />
                  </button>
                  <button
                    class="p-1.5 rounded-lg hover:bg-error/10 text-base-content/60 hover:text-error transition-colors"
                    onclick={() => openDeleteConfirm(agent)}
                    title={t("common.delete")}
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>

              {#if agent.systemPrompt}
                <p class="text-sm text-base-content/70 line-clamp-2 mb-3">
                  {agent.systemPrompt}
                </p>
              {/if}

              {#if agent.skills.length > 0}
                <div class="flex flex-wrap gap-1">
                  {#each agent.skills as skill}
                    <span
                      class="px-2 py-0.5 text-xs rounded-full bg-info/20 text-info"
                    >
                      {skill}
                    </span>
                  {/each}
                </div>
              {/if}

              {#if getGenuiName(agent)}
                <div class="mt-2 flex items-center gap-1 text-xs text-primary">
                  <LayoutTemplate size={12} />
                  {getGenuiName(agent)}
                </div>
              {/if}

              <div class="mt-3 pt-3 border-t border-base-300 text-xs text-base-content/50">
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
