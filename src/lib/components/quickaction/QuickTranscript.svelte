<!--
  Quick Action 浮层「answered 步」的一回合 transcript。

  复用与主对话完全一致的助手消息渲染(MessageAssistant:markdown / reasoning / 工具
  调用 / json-render / 流式),用户消息则用一个轻量右对齐气泡(无重发/编辑等按钮——
  浮层是一回合文档,要继续请「在对话中继续」)。

  数据源是 messageStore(浮层 webview 独立的单例,与主窗口互不影响),按 `chatId`
  分键读取消息 + 全局流式状态;流式中的助手消息按 ChatContent 的方式临时合成。
-->
<script lang="ts">
  import { messageStore } from "$lib/states/message.svelte";
  import { chatState } from "$lib/states/chat.svelte";
  import MessageAssistant from "$lib/components/chat/messages/MessageAssistant.svelte";
  import type { Message } from "$lib/types";

  interface Props {
    chatId: string;
  }

  let { chatId }: Props = $props();

  let messages = $derived(messageStore.getMessagesReactive(chatId));
  let streamingContent = $derived(messageStore.streamingContent);
  let streamingReasoning = $derived(messageStore.streamingReasoning);
  let streamingToolCalls = $derived(messageStore.streamingToolCalls);
  let streamingMessageId = $derived(messageStore.streamingMessageId);
  let isReasoning = $derived(messageStore.isReasoning);
  let isMessageLoading = $derived(messageStore.isMessageLoading);

  // 流式中的助手消息:用 messageStore 的临时流式状态合成一条 Message(完成后由
  // finishStreaming 落入 messagesByChat,这条临时消息随即消失)。
  let streamingMessage = $derived({
    id: streamingMessageId ?? "streaming",
    sessionId: chatId,
    role: "assistant" as const,
    content: streamingContent ?? "",
    reasoning: streamingReasoning,
    toolCalls: streamingToolCalls
      ? streamingToolCalls.map((call, index) => ({
          index: call.index ?? index,
          id: call.id,
          toolType: call.toolType,
          function: call.function,
        }))
      : undefined,
    config: {
      modelId: chatState.currentChat?.modelId,
      providerId: chatState.currentChat?.providerId,
    },
    createdAt: Date.now(),
    updatedAt: Date.now(),
  } as Message);

  let container: HTMLDivElement;

  function scrollToBottom(): void {
    if (container) container.scrollTop = container.scrollHeight;
  }

  // 消息追加或流式增量时贴底,使最新内容始终可见。
  $effect(() => {
    if (messages.length > 0 || streamingContent || streamingToolCalls) {
      setTimeout(scrollToBottom, 50);
    }
  });
</script>

<div bind:this={container} class="space-y-5 px-4 py-3.5">
  {#each messages as message (message.id)}
    {#if message.role === "user"}
      <div class="flex justify-end">
        <div
          class="inline-block max-w-full whitespace-pre-wrap break-words rounded-lg border border-[var(--hairline)] bg-base-200 px-3.5 py-2 text-[15px] leading-[1.6]"
        >
          {message.content}
        </div>
      </div>
    {:else if message.role === "assistant"}
      <MessageAssistant {message} />
    {/if}
  {/each}

  {#if isMessageLoading || (streamingMessageId && (streamingContent || streamingReasoning || streamingToolCalls))}
    <MessageAssistant
      message={streamingMessage}
      isStreaming={true}
      isReasoning={!!isReasoning}
      {isMessageLoading}
    />
  {/if}
</div>
