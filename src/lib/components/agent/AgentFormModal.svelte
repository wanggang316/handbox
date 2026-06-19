<script lang="ts">
  import Modal from "../ui/Modal.svelte";
  import Input from "../ui/Input.svelte";
  import Button from "../ui/Button.svelte";
  import TableGroup from "../ui/table/TableGroup.svelte";
  import TableBaseRow from "../ui/table/TableBaseRow.svelte";
  import SwitchRow from "../ui/table/SwitchRow.svelte";
  import { Bot } from "@lucide/svelte";
  import { t } from "$lib/i18n";
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
    generativeUI: boolean;
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
    generativeUI: false,
  });

  let saving = $state(false);

  async function handleSave() {
    if (!formData.name.trim()) {
      alert(t("agent.form.nameRequired"));
      return;
    }

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
        generativeUI: agent.generativeUi ?? false,
      };
    } else {
      formData = {
        name: "",
        model: "",
        systemPrompt: "",
        skills: "",
        generativeUI: false,
      };
    }
  });
</script>

<Modal bind:open={localOpen} title={agent ? t("agent.form.editTitle") : t("agent.form.createTitle")} onClose={onClose}>
  <div class="w-[600px] max-h-[80vh] overflow-y-auto px-6 pt-16 pb-6 flex flex-col gap-5">
    <!-- 基本信息 -->
    <TableGroup>
      <TableBaseRow label={t("agent.form.nameLabel")} layout="vertical">
        <Input
          placeholder={t("agent.form.namePlaceholder")}
          bind:value={formData.name}
        />
      </TableBaseRow>
    </TableGroup>

    <!-- 模型选择 -->
    <TableGroup>
      <TableBaseRow label={t("agent.form.modelLabel")} layout="vertical">
        <Input
          placeholder={t("agent.form.modelPlaceholder")}
          bind:value={formData.model}
        />
        <p class="mt-1 text-xs text-base-content/50">
          {t("agent.form.modelHint")}
        </p>
      </TableBaseRow>
    </TableGroup>

    <!-- 系统提示词 -->
    <TableGroup title={t("agent.form.systemPromptTitle")}>
      <div class="px-6">
        <textarea
          bind:value={formData.systemPrompt}
          placeholder={t("agent.systemPrompt.placeholder")}
          rows="4"
          class="w-full px-3 py-2 border border-base-300 rounded-md resize-none
                 focus:border-transparent
                 font-mono text-sm text-base-content bg-base-300"
        ></textarea>
        <div class="mt-1 text-xs text-base-content/50 text-right">
          {t("agent.form.charCount", { count: formData.systemPrompt.length })}
        </div>
      </div>
    </TableGroup>

    <!-- 技能 -->
    <TableGroup title={t("agent.form.skillsTitle")}>
      <TableBaseRow label={t("agent.form.skillsLabel")} layout="vertical">
        <input
          type="text"
          bind:value={formData.skills}
          placeholder={t("agent.form.skillsPlaceholder")}
          class="w-full px-3 py-2 border border-base-300 rounded-md
                 focus:border-transparent
                 text-sm text-base-content bg-base-300"
        />
        <p class="mt-1 text-xs text-base-content/50">
          {t("agent.form.skillsHint")}
        </p>
      </TableBaseRow>
    </TableGroup>

    <!-- 生成式 UI -->
    <TableGroup>
      <SwitchRow
        label="生成式 UI"
        description="允许助手在回复中渲染交互式界面"
        bind:checked={formData.generativeUI}
      />
    </TableGroup>

    <!-- 模型参数 - 始终显示 -->
    <TableGroup title={t("agent.form.modelParams")} collapsible defaultCollapsed={true}>
      <div class="px-6 space-y-3">
        <TableBaseRow label="Temperature">
          <input
            type="number"
            step="0.1"
            min="0"
            max="2"
            placeholder="0.7"
            class="w-full px-3 py-2 border border-base-300 rounded-md
                   focus:border-transparent
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
                   focus:border-transparent
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
                   focus:border-transparent
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
                   focus:border-transparent
                   text-sm text-base-content bg-base-300"
            bind:value={formData.maxTokens}
          />
        </TableBaseRow>
      </div>
    </TableGroup>

    <!-- MCP 服务器 - 始终显示 -->
    <TableGroup title={t("agent.form.mcpServers")} collapsible defaultCollapsed={true}>
      <div class="px-6">
        <p class="text-sm text-base-content/60">
          {t("agent.form.mcpComingSoon")}
        </p>
      </div>
    </TableGroup>

    <!-- 底部按钮 -->
    <div class="flex items-center justify-end gap-3 pt-4 border-t border-base-300">
      <Button
        variant="ghost"
        onclick={onClose}
        disabled={saving}
      >
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
