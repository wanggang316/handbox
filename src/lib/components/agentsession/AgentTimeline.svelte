<script lang="ts">
  import { renderMarkdown, markdownInteractions } from "$lib/utils";
  import { agentRunStore } from "$lib/states/agentRun.svelte";
  import type {
    AgentMessage,
    ToolResultContent,
  } from "$lib/types/agentSession";
  import AgentThinkingBlock from "./AgentThinkingBlock.svelte";
  import AgentToolCallCard from "./AgentToolCallCard.svelte";

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();

  // 会话运行 view-model（响应式 getter；按 sessionId 分键）。
  const runState = $derived(agentRunStore.runStateFor(sessionId));

  // 从用户消息提取纯文本（content 为字符串或内容块数组）。
  function userText(message: Extract<AgentMessage, { role: "user" }>): string {
    if (typeof message.content === "string") {
      return message.content;
    }
    return message.content
      .map((block) => (block.type === "text" ? block.text : ""))
      .join("");
  }

  // 拼接助手消息中所有 text 块（thinking / toolcall 块单独渲染）。
  function assistantText(
    message: Extract<AgentMessage, { role: "assistant" }>,
  ): string {
    return message.content
      .map((block) => (block.type === "text" ? block.text : ""))
      .join("");
  }

  // 提取助手消息中所有 thinking 块文本（present-only：无内容则为空）。
  function assistantThinking(
    message: Extract<AgentMessage, { role: "assistant" }>,
  ): string {
    return message.content
      .map((block) => (block.type === "thinking" ? block.thinking : ""))
      .join("");
  }

  // 助手消息中的工具调用块，按助手内容的**源顺序**保留（多个并行工具调用据此
  // 渲染为顺序排列的卡片，而非按完成顺序——VAL-TOOLS-004）。
  function assistantToolCalls(
    message: Extract<AgentMessage, { role: "assistant" }>,
  ) {
    return message.content.filter((block) => block.type === "toolcall");
  }

  // 已提交 transcript 里的 toolResult 消息，按 toolCallId 建索引，供 restored
  // 路径（reload 后无 live 状态）把 toolcall 块调和成终态卡片。run 期间 live 状态
  // 优先，此索引仅在 live 缺失时兜底（reconcile：同一 toolCallId 一张卡）。
  const committedToolResults = $derived.by(() => {
    const map = new Map<
      string,
      { content: ToolResultContent[]; isError: boolean }
    >();
    for (const message of runState.messages) {
      if (message.role === "toolResult") {
        map.set(message.toolCallId, {
          content: message.content,
          isError: message.isError,
        });
      }
    }
    return map;
  });

  // 把一个助手 toolcall 块归一化为卡片消费的 view-model：live 优先，restored 兜底。
  function toolCallView(block: Extract<AgentMessage, { role: "assistant" }>["content"][number]) {
    if (block.type !== "toolcall") {
      throw new Error("toolCallView expects a toolcall block");
    }
    return agentRunStore.toolCallViewFor(
      sessionId,
      block.id,
      block.name,
      block.arguments,
      committedToolResults.get(block.id),
    );
  }

  // 运行中追加的「进行中助手骨架」索引：reducer 在 message_start 时即把助手消息
  // 追加到 messages（内容尚空、usage 为零），其增量走 streamingText/thinkingText。
  // 该骨架由下方 LIVE 视图负责呈现，故此处需抑制其空内容/零用量的重复渲染。
  //
  // 仅当「最后一条助手消息尚无已提交内容」时才抑制它（交由 LIVE 视图）。一旦
  // message_end 把真实内容写入该助手消息，就返回 -1（不抑制），使已完成消息
  // 立即从已提交序列渲染——避免 message_end 与 agent_stream_closed 之间出现
  // 答案→pulse-dot→答案的闪烁（此区间 isRunning 仍为 true 而 streamingText 已清空）。
  const liveAssistantIndex = $derived.by(() => {
    if (!runState.isRunning) {
      return -1;
    }
    const last = runState.messages.length - 1;
    if (last < 0 || runState.messages[last].role !== "assistant") {
      return -1;
    }
    const lastMsg = runState.messages[last] as Extract<
      AgentMessage,
      { role: "assistant" }
    >;
    // 含工具调用块的助手消息也算「有内容」：其工具卡片在已提交分支渲染（卡片
      // 据 toolCallId 调和 live 状态而就地翻转），不能当作空骨架交给 LIVE 视图，
      // 否则工具执行期间卡片会被 pulse-dot 顶掉（VAL-TOOLS-004）。
    const hasContent =
      assistantText(lastMsg).length > 0 ||
      assistantThinking(lastMsg).length > 0 ||
      assistantToolCalls(lastMsg).length > 0;
    return hasContent ? -1 : last;
  });

  // LIVE 流式视图仅在「运行中且尚无可显示的已完成内容」时呈现：
  // 要么有正在流式累积的文本/思考，要么最后一条助手骨架仍为空（liveAssistantIndex>=0）。
  // 一旦助手消息已完成（liveAssistantIndex===-1）且流式已清空，便不再显示 LIVE 视图，
  // 从而消除 message_end 与 agent_stream_closed 之间的 pulse-dot 闪烁。
  const showLiveView = $derived(
    runState.isRunning &&
      (!!runState.streamingText ||
        !!runState.thinkingText ||
        liveAssistantIndex >= 0),
  );

  // 自动滚动到底部（镜像 ChatContent 行为）。
  let messagesContainer: HTMLDivElement;

  function scrollToBottom() {
    if (messagesContainer) {
      messagesContainer.scrollTop = messagesContainer.scrollHeight;
    }
  }

  // 已提交消息数量变化时滚动。
  $effect(() => {
    if (runState.messages.length > 0) {
      setTimeout(scrollToBottom, 100);
    }
  });

  // 流式文本/思考增长时滚动。
  $effect(() => {
    if (runState.streamingText || runState.thinkingText) {
      setTimeout(scrollToBottom, 50);
    }
  });
</script>

<div bind:this={messagesContainer} class="flex-1 overflow-y-auto">
  <div class="w-full mx-auto max-w-[800px] py-4 px-1 space-y-6">
    <!-- 已提交消息（按顺序；多轮历史保留在上方）。messages 为 append-only 序列
         （reducer 仅追加 / 就地 finalize 同 index，不重排），故 index key 的 DOM
         复用稳定；卡片自身按 toolCallId 分键，in-place 状态不随消息 index 错位。 -->
    {#each runState.messages as message, i (i)}
      {#if message.role === "user"}
        <!-- 用户气泡（纯文本）。 -->
        <div class="flex justify-end">
          <div class="flex flex-col items-end">
            <div
              class="inline-block max-w-full px-3.5 py-2 rounded-lg bg-base-200 text-base-content border border-[var(--hairline)]"
            >
              <div
                class="whitespace-pre-wrap break-words text-[15px] leading-[1.6] text-left"
              >
                {userText(message)}
              </div>
            </div>
          </div>
        </div>
      {:else if message.role === "assistant" && i !== liveAssistantIndex}
        <!-- 助手消息（已完成）：思考块（present-only）+ markdown 文本 + 工具调用卡片 + 用量。 -->
        <!-- 运行中的进行中助手骨架由下方 LIVE 视图呈现，此处跳过以免重复渲染。 -->
        <div class="flex flex-col gap-2">
          <div class="flex-1 min-w-0">
            {#if assistantThinking(message)}
              <AgentThinkingBlock thinking={assistantThinking(message)} />
            {/if}

            {#if assistantText(message)}
              <div
                class="flex-1 break-words text-[15px] leading-[1.6] markdown-content"
                use:markdownInteractions
              >
                {@html renderMarkdown(assistantText(message))}
              </div>
            {/if}

            <!-- 工具调用卡片：按助手内容源顺序渲染；同一 toolCallId 一张卡，
                 就地从 executing 翻转到终态（live），reload 后由 committed
                 toolResult 调和（restored）。stable key = toolCallId。 -->
            {#if assistantToolCalls(message).length}
              <div class="mt-2 space-y-2">
                {#each assistantToolCalls(message) as block (block.id)}
                  <AgentToolCallCard toolCall={toolCallView(block)} />
                {/each}
              </div>
            {/if}

            <!-- 错误态消息（stopReason=error 携带 errorMessage）。 -->
            {#if message.stopReason === "error" && message.errorMessage}
              <div
                class="mt-2 px-3 py-2 rounded-md bg-error/10 text-error text-sm whitespace-pre-wrap break-words"
              >
                {message.errorMessage}
              </div>
            {/if}

            <!-- Token 用量（输入/输出）。 -->
            {#if message.usage}
              <div class="mt-2 flex flex-row gap-2 text-xs text-base-content/50">
                <span>输入 {message.usage.input}</span>
                <span>·</span>
                <span>输出 {message.usage.output}</span>
              </div>
            {/if}
          </div>
        </div>
      {/if}
      <!-- toolResult 消息不单独渲染：其内容由配对的 toolcall 卡片（按 toolCallId
           调和）在助手回合内就地呈现，避免「答案 + 游离的工具结果块」割裂。 -->

      <!-- 运行中的进行中助手骨架（i === liveAssistantIndex）有意不在此渲染，由下方 LIVE 视图呈现。 -->
    {/each}

    <!-- LIVE 流式视图：运行中展示增长的思考块 + 流式文本。 -->
    {#if showLiveView}
      <div class="flex flex-col gap-2">
        <div class="flex-1 min-w-0">
          {#if runState.thinkingText}
            <AgentThinkingBlock thinking={runState.thinkingText} isStreaming />
          {/if}

          {#if runState.streamingText}
            <div
              class="flex-1 break-words text-[15px] leading-[1.6] markdown-content"
              use:markdownInteractions
            >
              {@html renderMarkdown(runState.streamingText)}
            </div>
          {:else if !runState.thinkingText}
            <!-- 流式启动但尚无内容：进行中指示。 -->
            <div class="py-2 text-base-content flex items-center">
              <div
                class="h-4 w-4 rounded-full bg-current animate-[pulse-scale_1.5s_ease-in-out_infinite]"
              ></div>
            </div>
          {/if}
        </div>
      </div>
    {/if}

    <!-- 错误态：run-level 错误可见，而非静默停止。 -->
    {#if runState.error}
      <div
        class="px-3 py-2 rounded-md bg-error/10 text-error text-sm whitespace-pre-wrap break-words"
      >
        {runState.error}
      </div>
    {/if}
  </div>
</div>

<style>
  /* 自定义滚动条（镜像 ChatContent）。 */
  .overflow-y-auto::-webkit-scrollbar {
    width: 6px;
  }

  .overflow-y-auto::-webkit-scrollbar-track {
    background: transparent;
  }

  .overflow-y-auto::-webkit-scrollbar-thumb {
    background: color-mix(in oklch, var(--base-content) 15%, transparent);
    border-radius: 3px;
  }

  .overflow-y-auto::-webkit-scrollbar-thumb:hover {
    background: color-mix(in oklch, var(--base-content) 25%, transparent);
  }
</style>
