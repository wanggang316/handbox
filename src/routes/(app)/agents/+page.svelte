<script lang="ts">
  import { onMount } from "svelte";
  import { Plus, Bot, Pencil, Trash2, Settings, Play } from "@lucide/svelte";
  import { goto } from "$app/navigation";
  import { agentState, agentActions } from "$lib/states/agent.svelte";
  import type { Agent } from "$lib/types";
  import ConfirmModal from "$lib/components/ui/ConfirmModal.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import AgentFormModal from "$lib/components/agent/AgentFormModal.svelte";
  import type { AgentFormData } from "$lib/components/agent/AgentFormModal.svelte";
  import { createSessionFromAgent } from "$lib/api/chat";

  let searchQuery = $state("");

  let showFormModal = $state(false);
  let editingAgent = $state<Agent | null>(null);
  let showDeleteConfirm = $state(false);
  let selectedAgent = $state<Agent | null>(null);

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
    return agent.model || "未设置";
  }

  onMount(async () => {
    await agentActions.loadAgents();
  });
</script>

<div class="h-full flex flex-col">
  <div class="flex-shrink-0 p-4 border-b border-base-300 mt-12">
    <div class="flex items-center justify-between mb-4">
      <div class="flex items-center gap-4">
        <h1 class="text-xl font-semibold text-base-content flex items-center gap-2">
          <Bot size={24} />
          Agents
        </h1>
        <span class="text-sm text-base-content/60">
          {filteredAgents.length} 个
        </span>
      </div>
      <Button
        variant="primary"
        size="sm"
        onclick={openCreateModal}
        customClass="flex items-center gap-2"
      >
        <Plus size={16} />
        新建 Agent
      </Button>
    </div>

    <div class="relative">
      <input
        type="text"
        placeholder="搜索 Agent 名称或技能..."
        class="w-full h-9 pl-10 pr-4 bg-base-200 rounded-lg text-base-content placeholder:text-base-content/50 focus:outline-none focus:ring-2 focus:ring-primary/50 text-sm"
        bind:value={searchQuery}
      />
      <Settings
        class="absolute left-3 top-1/2 -translate-y-1/2 text-base-content/50"
        size={16}
      />
    </div>
  </div>

  <div class="flex-1 min-h-0 overflow-y-auto p-4">
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
          <p class="mb-2">没有找到匹配的 Agent</p>
          <button
            class="text-primary hover:underline cursor-pointer"
            on:click={() => (searchQuery = "")}
          >
            清除搜索
          </button>
        {:else}
          <p>还没有创建任何 Agent</p>
          <p class="text-sm mt-2">点击上方按钮创建您的第一个 Agent</p>
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
                  on:click={() => handleUseAgent(agent)}
                  title="使用"
                >
                  <Play size={14} />
                </button>
                <button
                  class="p-1.5 rounded-lg hover:bg-base-100 text-base-content/60 hover:text-base-content transition-colors"
                  on:click={() => openEditModal(agent)}
                  title="编辑"
                >
                  <Pencil size={14} />
                </button>
                <button
                  class="p-1.5 rounded-lg hover:bg-error/10 text-base-content/60 hover:text-error transition-colors"
                  on:click={() => openDeleteConfirm(agent)}
                  title="删除"
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

            <div class="mt-3 pt-3 border-t border-base-300 text-xs text-base-content/50">
              {new Date(agent.createdAt).toLocaleDateString("zh-CN")}
            </div>
          </div>
        {/each}
      </div>
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

<!-- 删除确认模态框 -->
<ConfirmModal
  title="删除 Agent"
  message="确定要删除这个 Agent 吗？此操作无法撤销。"
  confirmText="删除"
  confirmButtonStyle="danger"
  open={showDeleteConfirm}
  onClose={() => (showDeleteConfirm = false)}
  onConfirm={handleDelete}
/>
