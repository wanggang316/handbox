<script lang="ts">
  import { ArrowUp, Square } from "@lucide/svelte";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import ChatModelSelectButton from "$lib/components/chat/ChatModelSelectButton.svelte";
  import { agentSessionActions } from "$lib/states/agentSession.svelte";
  import { agentRunStore } from "$lib/states/agentRun.svelte";
  import { getAllModels } from "$lib/states/provider.svelte";
  import { runAgentStream } from "$lib/api/agentSession";
  import type { AgentSession } from "$lib/types";
  import type { ModelWithProvider } from "$lib/types/provider";

  interface Props {
    session: AgentSession;
  }

  let { session }: Props = $props();

  // 思考强度档位（与 AgentSessionCreateModal 保持一致；thinkingLevel 为后端自由文本字段）。
  const thinkingLevelOptions = [
    { value: "off", label: "关闭" },
    { value: "low", label: "低" },
    { value: "medium", label: "中" },
    { value: "high", label: "高" },
  ];

  let input = $state("");
  let textareaRef: HTMLTextAreaElement;
  let modelPrompt = $state<string | null>(null);

  // 会话存的是 modelId/providerId；picker 需要 ModelWithProvider，故从目录反查。
  const selectedModel = $derived<ModelWithProvider | null>(
    session.modelId && session.providerId
      ? (getAllModels().find(
          (m) =>
            m.id === session.modelId && m.provider_id === session.providerId,
        ) ?? null)
      : null,
  );

  const thinkingLevel = $derived(session.thinkingLevel ?? "off");

  // 该会话是否存在活跃 run —— 驱动 Send <-> Stop 切换（VAL-RUN-006）。
  const running = $derived(agentRunStore.isRunning(session.id));

  function adjustTextareaHeight() {
    if (textareaRef) {
      textareaRef.style.height = "auto";
      const scrollHeight = textareaRef.scrollHeight;
      const maxHeight = 200;
      textareaRef.style.height = Math.min(scrollHeight, maxHeight) + "px";
    }
  }

  // Enter 发送；Shift+Enter 换行（镜像 ChatInput）（VAL-RUN-011）。
  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      sendAgentRun();
    }
  }

  async function sendAgentRun() {
    // 空/纯空白输入为 no-op：不发起 run，不产生气泡（VAL-RUN-010）。
    if (!input.trim()) return;

    // 无模型则提示并阻断（防御性；创建会话通常已含模型）（VAL-RUN-010）。
    if (!session.modelId || !session.providerId) {
      modelPrompt = "请先选择模型";
      return;
    }

    modelPrompt = null;
    const text = input;
    // 先清空输入框；用户气泡由后端 emit 的 user message_end 经 agentRunStore reduce 出现，
    // 此处不做乐观插入以免重复。
    input = "";
    adjustTextareaHeight();
    try {
      await runAgentStream(session.id, text);
    } catch (error) {
      // 启动失败：回填输入，提示错误，便于重试。
      input = text;
      adjustTextareaHeight();
      modelPrompt =
        error instanceof Error ? error.message : "启动 Agent 运行失败";
    }
  }

  async function handleStop() {
    try {
      await agentRunStore.abort(session.id);
    } catch (error) {
      console.error("Failed to abort agent run:", error);
    }
  }

  function handleModelSelect(model: ModelWithProvider) {
    modelPrompt = null;
    agentSessionActions
      .updateField(session.id, "modelId", model.id)
      .then(() =>
        agentSessionActions.updateField(
          session.id,
          "providerId",
          model.provider_id,
        ),
      )
      .catch((error) => {
        console.error("Failed to update agent session model:", error);
      });
  }

  function handleThinkingChange(value: string) {
    agentSessionActions
      .updateField(session.id, "thinkingLevel", value)
      .catch((error) => {
        console.error("Failed to update agent session thinking level:", error);
      });
  }
</script>

<div
  class="flex flex-col bg-base-300 rounded-lg border border-[var(--hairline)] mx-auto w-full max-w-[800px]"
>
  <textarea
    bind:this={textareaRef}
    bind:value={input}
    placeholder="在这里输入消息，按 Enter 发送"
    onkeydown={handleKeydown}
    oninput={adjustTextareaHeight}
    rows="1"
    class="bg-transparent text-[14px] text-base-content/80 p-4 outline-none resize-none w-full min-h-[48px] max-h-[200px] overflow-y-auto"
  ></textarea>

  {#if modelPrompt}
    <div class="px-4 pb-1 text-xs text-warning">
      {modelPrompt}
    </div>
  {/if}

  <div class="flex flex-row items-center justify-end gap-3 px-4 pt-0 pb-2">
    <Select
      value={thinkingLevel}
      options={thinkingLevelOptions}
      size="sm"
      autoWidth
      onChange={handleThinkingChange}
    />
    <ChatModelSelectButton
      {selectedModel}
      onModelSelect={handleModelSelect}
    />
    {#if running}
      <CircleButton
        icon={Square}
        iconSize={16}
        size="w-8 h-8"
        ariaLabel="停止"
        onclick={handleStop}
      />
    {:else}
      <CircleButton
        icon={ArrowUp}
        iconSize={18}
        size="w-8 h-8"
        ariaLabel="发送"
        onclick={sendAgentRun}
      />
    {/if}
  </div>
</div>
