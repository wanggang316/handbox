<script lang="ts">
  import Modal from "../ui/Modal.svelte";
  import Input from "../ui/Input.svelte";
  import Button from "../ui/Button.svelte";
  import TableGroup from "../ui/table/TableGroup.svelte";
  import TableBaseRow from "../ui/table/TableBaseRow.svelte";
  import { Bot, ChevronDown } from "@lucide/svelte";
  import { getProviderIconById, providerState } from "$lib/states/provider.svelte";
  import ChatModelSelectModal from "../chat/ChatModelSelectModal.svelte";
  import type { Agent, ModelWithProvider } from "$lib/types";

  interface Props {
    open: boolean;
    agent: Agent | null;
    onClose: () => void;
    onSave: (data: AgentFormData) => Promise<void>;
  }

  export interface AgentFormData {
    name: string;
    modelId: string;
    providerId: string;
    temperature?: number;
    topP?: number;
    topK?: number;
    maxTokens?: number;
    streaming?: boolean;
    systemPrompt: string;
    skills: string;
  }

  let { open, agent, onClose, onSave }: Props = $props();

  // 本地状态，因为 props 是只读的
  let localOpen = $state(false);

  // 同步外部 open 状态到本地状态
  $effect(() => {
    localOpen = open;
  });

  let formData = $state<AgentFormData>({
    name: "",
    modelId: "",
    providerId: "",
    systemPrompt: "",
    skills: "",
  });

  let showModelModal = $state(false);
  let saving = $state(false);

  // 当前选择的模型
  const selectedModel = $derived<ModelWithProvider | undefined>(
    providerState.providersWithModels
      .flatMap((p) =>
        p.models.map(
          (m) =>
            ({
              ...m,
              providerName: p.name,
              providerType: p.provider_type,
            }) as ModelWithProvider
        )
      )
      .find((m) => m.id === formData.modelId && m.provider_id === formData.providerId)
  );

  const providerIcon = $derived(
    selectedModel?.provider_id
      ? getProviderIconById(selectedModel.provider_id)
      : undefined
  );

  function handleModelSelect(model: ModelWithProvider) {
    formData.modelId = model.id;
    formData.providerId = model.provider_id;
    showModelModal = false;
  }

  async function handleSave() {
    if (!formData.name.trim()) {
      alert("请输入 Agent 名称");
      return;
    }

    saving = true;
    try {
      await onSave(formData);
      localOpen = false;
      onClose();
    } catch (error) {
      console.error("Failed to save agent:", error);
      alert("保存失败，请重试");
    } finally {
      saving = false;
    }
  }

  // 当 agent 变化时，更新表单数据
  $effect(() => {
    if (agent) {
      formData = {
        name: agent.name,
        modelId: agent.modelId || "",
        providerId: agent.providerId || "",
        temperature: agent.temperature,
        topP: agent.topP,
        topK: agent.topK,
        maxTokens: agent.maxTokens,
        streaming: agent.streaming,
        systemPrompt: agent.systemPrompt || "",
        skills: agent.skills.join(", "),
      };
    } else {
      formData = {
        name: "",
        modelId: "",
        providerId: "",
        systemPrompt: "",
        skills: "",
      };
    }
  });
</script>

<Modal bind:open={localOpen} title={agent ? "编辑 Agent" : "新建 Agent"} onClose={onClose}>
  <div class="w-[600px] max-h-[80vh] overflow-y-auto px-6 pt-16 pb-6 flex flex-col gap-5">
    <!-- 基本信息 -->
    <TableGroup>
      <TableBaseRow label="名称" layout="vertical">
        <Input
          placeholder="输入 Agent 名称"
          bind:value={formData.name}
        />
      </TableBaseRow>
    </TableGroup>

    <!-- 模型选择 -->
    <TableGroup>
      <button
        class="w-full rounded-2xl bg-base-200 px-3 py-4 border border-base-200 hover:bg-base-300"
        type="button"
        onclick={() => (showModelModal = true)}
      >
        {#if selectedModel}
          <div class="flex items-start justify-between gap-2">
            <div class="space-y-1 pb-1 flex-1 flex flex-col text-left">
              <div class="flex flex-row justify-start items-center gap-2">
                {#if providerIcon}
                  <img
                    src={providerIcon}
                    alt={selectedModel?.providerName ?? "模型供应商"}
                    class="h-4 w-4 rounded-md object-contain"
                  />
                {/if}
                <p class="text-xs text-base-content/50">
                  {selectedModel?.providerName ?? "模型供应商"}
                </p>
              </div>

              <div class="text-md text-base-content">
                {selectedModel ? selectedModel.name : "未选择模型"}
              </div>
              <div
                class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-base-content/60"
              >
                {#if selectedModel?.id}
                  <span class="font-mono text-[11px] text-base-content/50">
                    {selectedModel.id}
                  </span>
                {/if}
              </div>
            </div>
          </div>
        {:else}
          <div class="flex flex-row justify-between items-center px-2">
            <p class="text-left text-sm leading-relaxed text-base-content">
              {formData.modelId ? formData.modelId : "选择一个模型"}
            </p>
            <ChevronDown size={14} />
          </div>
        {/if}
      </button>
    </TableGroup>

    <!-- 系统提示词 -->
    <TableGroup title="系统提示词">
      <div class="px-6">
        <textarea
          bind:value={formData.systemPrompt}
          placeholder="输入系统提示词..."
          rows="4"
          class="w-full px-3 py-2 border border-base-300 rounded-md resize-none
                 focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent
                 font-mono text-sm text-base-content bg-base-200"
        ></textarea>
        <div class="mt-1 text-xs text-base-content/50 text-right">
          {formData.systemPrompt.length} 字符
        </div>
      </div>
    </TableGroup>

    <!-- 技能 -->
    <TableGroup title="技能">
      <TableBaseRow label="技能标签" layout="vertical">
        <input
          type="text"
          bind:value={formData.skills}
          placeholder="例如: coding, writing, translation"
          class="w-full px-3 py-2 border border-base-300 rounded-md
                 focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent
                 text-sm text-base-content bg-base-200"
        />
        <p class="mt-1 text-xs text-base-content/50">
          用逗号分隔多个技能标签
        </p>
      </TableBaseRow>
    </TableGroup>

    <!-- 模型参数 - 始终显示 -->
    <TableGroup title="模型参数" collapsible defaultCollapsed={true}>
      <div class="px-6 space-y-3">
        <TableBaseRow label="Temperature">
          <input
            type="number"
            step="0.1"
            min="0"
            max="2"
            placeholder="0.7"
            class="w-full px-3 py-2 border border-base-300 rounded-md
                   focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent
                   text-sm text-base-content bg-base-200"
            bind:value={formData.temperature}
          />
        </TableBaseRow>

        <TableBaseRow label="Top P">
          <input
            type="number"
            step="0.1"
            min="0"
            max="1"
            placeholder="0.9"
            class="w-full px-3 py-2 border border-base-300 rounded-md
                   focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent
                   text-sm text-base-content bg-base-200"
            bind:value={formData.topP}
          />
        </TableBaseRow>

        <TableBaseRow label="Top K">
          <input
            type="number"
            min="1"
            placeholder="40"
            class="w-full px-3 py-2 border border-base-300 rounded-md
                   focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent
                   text-sm text-base-content bg-base-200"
            bind:value={formData.topK}
          />
        </TableBaseRow>

        <TableBaseRow label="Max Tokens">
          <input
            type="number"
            min="1"
            placeholder="2048"
            class="w-full px-3 py-2 border border-base-300 rounded-md
                   focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent
                   text-sm text-base-content bg-base-200"
            bind:value={formData.maxTokens}
          />
        </TableBaseRow>

        <TableBaseRow label="Streaming">
          <div class="flex items-center gap-2">
            <input
              type="checkbox"
              id="streaming"
              class="w-4 h-4 rounded border-base-300 text-primary focus:ring-primary"
              bind:checked={formData.streaming}
            />
            <label for="streaming" class="text-sm text-base-content">
              启用流式输出
            </label>
          </div>
        </TableBaseRow>
      </div>
    </TableGroup>

    <!-- MCP 服务器 - 始终显示 -->
    <TableGroup title="MCP 服务器" collapsible defaultCollapsed={true}>
      <div class="px-6">
        <p class="text-sm text-base-content/60">
          MCP 服务器配置即将推出，敬请期待...
        </p>
      </div>
    </TableGroup>

    <!-- 底部按钮 -->
    <div class="flex items-center justify-end gap-3 pt-4 border-t border-base-300">
      <Button
        variant="ghost"
        on:click={onClose}
        disabled={saving}
      >
        取消
      </Button>
      <Button
        variant="primary"
        on:click={handleSave}
        disabled={saving || !formData.name.trim()}
      >
        {saving ? "保存中..." : agent ? "保存" : "创建"}
      </Button>
    </div>
  </div>
</Modal>

<ChatModelSelectModal
  bind:open={showModelModal}
  selectedModel={selectedModel ?? null}
  onModelSelect={handleModelSelect}
/>
