<script lang="ts">
  import Modal from "../ui/Modal.svelte";
  import Input from "../ui/Input.svelte";
  import Button from "../ui/Button.svelte";
  import TableGroup from "../ui/table/TableGroup.svelte";
  import TableBaseRow from "../ui/table/TableBaseRow.svelte";
  import { Bot } from "@lucide/svelte";
  import type { Agent } from "$lib/types";

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
    model: "",
    systemPrompt: "",
    skills: "",
  });

  let saving = $state(false);

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
        model: agent.model || "",
        temperature: agent.temperature,
        topP: agent.topP,
        topK: agent.topK,
        maxTokens: agent.maxTokens,
        systemPrompt: agent.systemPrompt || "",
        skills: agent.skills.join(", "),
      };
    } else {
      formData = {
        name: "",
        model: "",
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
      <TableBaseRow label="模型" layout="vertical">
        <Input
          placeholder="输入模型标识符 (例如: gpt-4, claude-3-5-sonnet-20241022)"
          bind:value={formData.model}
        />
        <p class="mt-1 text-xs text-base-content/50">
          模型标识符可以是任何字符串，不限于已配置的模型
        </p>
      </TableBaseRow>
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
                 font-mono text-sm text-base-content bg-base-300"
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
                 text-sm text-base-content bg-base-300"
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
                   text-sm text-base-content bg-base-300"
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
                   text-sm text-base-content bg-base-300"
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
                   text-sm text-base-content bg-base-300"
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
                   text-sm text-base-content bg-base-300"
            bind:value={formData.maxTokens}
          />
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
