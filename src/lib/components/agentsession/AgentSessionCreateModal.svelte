<script lang="ts">
  import Modal from "$lib/components/ui/Modal.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import TableBaseRow from "$lib/components/ui/table/TableBaseRow.svelte";
  import ChatModelSelectButton from "$lib/components/chat/ChatModelSelectButton.svelte";
  import { FolderOpen } from "@lucide/svelte";
  import {
    providerState,
    providerActions,
    getAllModels,
  } from "$lib/states/provider.svelte";
  import { agentSessionActions } from "$lib/states/agentSession.svelte";
  import type { ModelWithProvider } from "$lib/types/provider";
  import type { AgentSession } from "$lib/types";

  interface Props {
    open?: boolean;
    onCreated?: (session: AgentSession) => void;
  }

  let { open = $bindable(false), onCreated = () => {} }: Props = $props();

  // 思考强度档位（thinkingLevel 为后端自由文本字段）。
  const thinkingLevelOptions = [
    { value: "off", label: "关闭" },
    { value: "low", label: "低" },
    { value: "medium", label: "中" },
    { value: "high", label: "高" },
  ];

  let name = $state("");
  let selectedModel = $state<ModelWithProvider | null>(null);
  let thinkingLevel = $state("off");
  let systemPrompt = $state("");
  let workingDir = $state("");
  let creating = $state(false);
  let errorMessage = $state<string | null>(null);

  // 是否存在已启用且含模型的供应商（决定模型选择器是否可用）。
  const hasModels = $derived(getAllModels().length > 0);

  // 打开时刷新供应商模型目录（与 ChatModelSelectModal 同策略），并重置表单。
  $effect(() => {
    if (!open) {
      return;
    }
    if (
      providerState.providersWithModelsNeedRefresh ||
      providerState.providersWithModels.length === 0
    ) {
      providerActions.loadProvidersWithModels().catch((err) => {
        console.error("Failed to load models for agent session:", err);
      });
    }
  });

  function resetForm() {
    name = "";
    selectedModel = null;
    thinkingLevel = "off";
    systemPrompt = "";
    workingDir = "";
    errorMessage = null;
  }

  function handleClose() {
    open = false;
    resetForm();
  }

  async function pickWorkingDir() {
    try {
      const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
      const dir = await openDialog({ directory: true });
      if (typeof dir === "string") {
        workingDir = dir;
      }
    } catch (error) {
      console.error("Failed to pick working directory:", error);
    }
  }

  async function handleCreate() {
    if (!hasModels || !selectedModel) {
      errorMessage = "请先配置供应商";
      return;
    }

    creating = true;
    errorMessage = null;
    try {
      const trimmedName = name.trim();
      const session = await agentSessionActions.createSession({
        name: trimmedName || "未命名",
        modelId: selectedModel.id,
        providerId: selectedModel.provider_id,
        systemPrompt: systemPrompt.trim() || undefined,
        thinkingLevel,
        workingDir: workingDir.trim() || undefined,
      });
      onCreated(session);
      open = false;
      resetForm();
    } catch (error) {
      errorMessage =
        error instanceof Error ? error.message : "创建 Agent 会话失败";
    } finally {
      creating = false;
    }
  }
</script>

<Modal bind:open title="新建 Agent 会话" onClose={handleClose}>
  <div
    class="w-[600px] max-h-[80vh] overflow-y-auto px-6 pt-16 pb-6 flex flex-col gap-5"
  >
    {#if !hasModels}
      <div
        class="px-4 py-3 rounded-md bg-warning/10 text-warning text-sm border border-warning/30"
      >
        尚未配置任何供应商，请先配置供应商后再新建 Agent 会话。
      </div>
    {/if}

    <!-- 名称 -->
    <TableGroup>
      <TableBaseRow label="名称" layout="vertical">
        <input
          type="text"
          bind:value={name}
          placeholder="留空则默认为「未命名」"
          class="w-full px-3 py-2 border border-base-300 rounded-md
                 focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent
                 text-sm text-base-content bg-base-300"
        />
      </TableBaseRow>
    </TableGroup>

    <!-- 模型 -->
    <TableGroup>
      <TableBaseRow label="模型" layout="vertical">
        {#if hasModels}
          <ChatModelSelectButton
            {selectedModel}
            variant="secondary"
            onModelSelect={(model) => (selectedModel = model)}
          />
        {:else}
          <Button variant="secondary" disabled>选择模型</Button>
        {/if}
      </TableBaseRow>
    </TableGroup>

    <!-- 思考强度 -->
    <TableGroup>
      <TableBaseRow label="思考强度" layout="vertical">
        <Select
          bind:value={thinkingLevel}
          options={thinkingLevelOptions}
          disabled={!hasModels}
        />
      </TableBaseRow>
    </TableGroup>

    <!-- 工作目录 -->
    <TableGroup>
      <TableBaseRow label="工作目录" layout="vertical">
        <div class="flex items-center gap-2">
          <input
            type="text"
            bind:value={workingDir}
            placeholder="选择一个工作目录"
            readonly
            class="flex-1 px-3 py-2 border border-base-300 rounded-md
                   focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent
                   text-sm text-base-content bg-base-300"
          />
          <Button variant="secondary" on:click={pickWorkingDir} disabled={!hasModels}>
            <FolderOpen size={14} />
            选择
          </Button>
        </div>
      </TableBaseRow>
    </TableGroup>

    <!-- 系统提示词 -->
    <TableGroup title="系统提示词">
      <div class="px-6">
        <textarea
          bind:value={systemPrompt}
          placeholder="输入系统提示词..."
          rows="4"
          disabled={!hasModels}
          class="w-full px-3 py-2 border border-base-300 rounded-md resize-none
                 focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent
                 font-mono text-sm text-base-content bg-base-300"
        ></textarea>
      </div>
    </TableGroup>

    {#if errorMessage}
      <div class="px-4 py-2 rounded-md bg-error/10 text-error text-sm">
        {errorMessage}
      </div>
    {/if}

    <!-- 底部按钮 -->
    <div
      class="flex items-center justify-end gap-3 pt-4 border-t border-base-300"
    >
      <Button variant="ghost" on:click={handleClose} disabled={creating}>
        取消
      </Button>
      <Button
        variant="primary"
        on:click={handleCreate}
        disabled={creating || !hasModels || !selectedModel}
      >
        {creating ? "创建中..." : "创建"}
      </Button>
    </div>
  </div>
</Modal>
