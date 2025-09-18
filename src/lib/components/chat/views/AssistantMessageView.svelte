<script lang="ts">
  import {
    Copy,
    RotateCcw,
    Trash2,
    ChevronDown,
    ChevronRight,
  } from "lucide-svelte";
  import type { Message } from "$lib/types";
  import { messageStore } from "$lib/states/message.svelte";
  import { renderMarkdown } from "$lib/utils";

  interface Props {
    message?: Message;
    isOperating?: boolean;
    isStreaming?: boolean;
    isReasoning?: boolean;
    isMessageLoading?: boolean;
    onCopy?: (content: string) => void;
    onRegenerate?: (messageId: string) => void;
    onDelete?: (messageId: string) => void;
  }

  let {
    message,
    isOperating = false,
    isStreaming = false,
    isReasoning = false,
    isMessageLoading = false,
    onCopy,
    onRegenerate,
    onDelete,
  }: Props = $props();

  // reasoning 折叠状态，流式消息默认收起，完成的消息默认展开
  let reasoningExpanded = $state(!isStreaming);

  // 获取provider配置
  const providerConfig = $derived(() => {
    if (message?.config?.providerId) {
      return messageStore.getProviderConfig(message.config.providerId);
    }
    return undefined;
  });

  // 格式化时间戳
  function formatTime(timestamp: number): string {
    return new Date(timestamp).toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  // 格式化持续时间
  function formatDuration(duration?: number): string {
    if (!duration) return "";

    if (duration < 1000) {
      return `${duration}ms`;
    } else {
      return `${(duration / 1000).toFixed(1)}s`;
    }
  }

  // 处理操作
  function handleCopy() {
    if (message?.content) {
      onCopy?.(message.content);
    }
  }

  function handleRegenerate() {
    if (message?.id) {
      onRegenerate?.(message.id);
    }
  }

  function handleDelete() {
    if (message?.id) {
      onDelete?.(message.id);
    }
  }

  function closestButton(target: EventTarget | null): HTMLButtonElement | null {
    if (!(target instanceof Element)) return null;
    return target.closest<HTMLButtonElement>(".markdown-code-block__copy");
  }

  async function copyText(content: string) {
    try {
      await navigator.clipboard.writeText(content);
    } catch (error) {
      const textarea = document.createElement("textarea");
      textarea.value = content;
      textarea.setAttribute("readonly", "");
      textarea.style.position = "absolute";
      textarea.style.left = "-9999px";
      document.body.appendChild(textarea);
      textarea.select();
      try {
        document.execCommand("copy");
      } catch (fallbackError) {
        console.error("Failed to copy code block", fallbackError);
      }
      document.body.removeChild(textarea);
    }
  }

  function markdownCopy(node: HTMLElement) {
    const handleClick = async (event: MouseEvent) => {
      const button = closestButton(event.target);
      if (!button) return;

      event.preventDefault();
      event.stopPropagation();

      const block = button.closest<HTMLElement>(".markdown-code-block");
      const codeElement = block?.querySelector("code");
      const codeContent = codeElement?.textContent ?? "";

      if (!codeContent) return;

      if (onCopy) {
        onCopy(codeContent);
      } else {
        await copyText(codeContent);
      }

      button.classList.add("copied");

      const timerId = button.dataset.copyTimeout
        ? Number(button.dataset.copyTimeout)
        : undefined;
      if (timerId) {
        window.clearTimeout(timerId);
      }

      const timeoutHandle = window.setTimeout(() => {
        button.classList.remove("copied");
        delete button.dataset.copyTimeout;
      }, 1500);

      button.dataset.copyTimeout = String(timeoutHandle);
    };

    node.addEventListener("click", handleClick);

    return {
      destroy() {
        node.removeEventListener("click", handleClick);
      },
    };
  }

  // 切换推理过程显示状态
  function toggleReasoning() {
    reasoningExpanded = !reasoningExpanded;
  }
</script>

<div class="group relative">
  <!-- 消息容器 -->
  <div class="flex flex-col gap-2">
    <!-- 模型供应商图标（模型） -->
    <div class="flex flex-row gap-2">
      <div
        class="w-8 h-8 rounded-full bg-gray-100 flex items-center justify-center"
      >
        <img
          src={providerConfig()?.icon}
          alt={providerConfig()?.type_name || "AI"}
          class="w-4 h-4 object-contain"
        />
      </div>

      {#if message?.config?.modelId}
        <div class="flex items-center gap-1 text-gray-400 text-xs">
          {message.config.modelId}
        </div>
      {/if}
    </div>

    <!-- 消息内容 -->
    <div class="flex-1 min-w-0">
      {#if isMessageLoading}
        <!-- 加载状态 -->
        <div class="max-w-full py-2 text-gray-900 flex items-center">
          <div class="h-4 w-4 rounded-full bg-current animate-[pulse-scale_1.5s_ease-in-out_infinite]"></div>
        </div>
      {:else}
        <!-- 消息气泡 -->
        <div class="max-w-full py-0 text-gray-900">
          <!-- 推理过程（如果有） -->
          {#if message?.reasoning}
            <div class="mb-4">
              <!-- 推理过程标题，可点击折叠 -->
              <button
                class="flex items-center gap-1 my-2 text-left hover:bg-bg-hover rounded-full py-1 px-2"
                onclick={toggleReasoning}
              >
                {#if reasoningExpanded}
                  <ChevronDown size={16} class="text-gray-600" />
                {:else}
                  <ChevronRight size={16} class="text-gray-600" />
                {/if}
                <span class="text-sm font-medium text-gray-600">
                  {isReasoning ? "推理中..." : "推理过程"}
                </span>
              </button>

              <!-- 推理过程内容，根据展开状态显示 -->
              {#if reasoningExpanded}
                <div
                  class="mt-2 mb-6 px-4 text-sm border-l border-gray-200 text-gray-600 break-words leading-relaxed reasoning-content markdown-content"
                  use:markdownCopy
                >
                  {@html renderMarkdown(message.reasoning)}
                </div>
              {/if}
            </div>
          {/if}

          <!-- 消息内容 -->
          <div
            class="flex-1 break-words text-[15px] leading-[1.6] markdown-content"
            use:markdownCopy
          >
            {@html renderMarkdown(message?.content || "")}
          </div>

          {#if !isStreaming && !isMessageLoading}
          <!-- 性能信息 -->
          <div class="flex flex-row gap-2 mt-6 text-xs text-gray-400">
            {#if message?.createdAt}
              <span>
                {formatTime(message.createdAt)}
              </span>
            {/if}
            {#if message?.inputTokens || message?.outputTokens || message?.totalTokens}
              <span class="font-medium">Token:</span>
              <span>
                {#if message.inputTokens}
                  | 输入: {message.inputTokens}{/if}
                {#if message.outputTokens}
                  | 输出: {message.outputTokens}{/if}
                {#if message.totalTokens}
                  | 总计: {message.totalTokens}{/if}
              </span>
            {/if}

            {#if message?.duration}
              <span> | 耗时: {formatDuration(message.duration)}</span>
            {/if}
          </div>
          {/if}
        </div>

        <!-- 消息操作按钮 (仅在非流式且非加载状态下显示) -->
        {#if !isStreaming && !isMessageLoading}
          <div
            class="mt-2 opacity-0 group-hover:opacity-100 transition-opacity duration-200"
          >
            <div class="inline-flex gap-1">
              <!-- 复制按钮 -->
              <button
                class="p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded transition-colors"
                title="复制消息"
                onclick={handleCopy}
              >
                <Copy class="w-3.5 h-3.5" />
              </button>

              <!-- 重新生成按钮 -->
              <button
                class="p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                title="重新生成"
                disabled={isOperating}
                onclick={handleRegenerate}
              >
                {#if isOperating}
                  <div
                    class="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin"
                  ></div>
                {:else}
                  <RotateCcw class="w-3.5 h-3.5" />
                {/if}
              </button>

              <!-- 删除按钮 -->
              <button
                class="p-1.5 text-gray-400 hover:text-red-600 hover:bg-red-50 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                title="删除消息"
                disabled={isOperating}
                onclick={handleDelete}
              >
                {#if isOperating}
                  <div
                    class="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin"
                  ></div>
                {:else}
                  <Trash2 class="w-3.5 h-3.5" />
                {/if}
              </button>
            </div>
          </div>
        {/if}
      {/if}
    </div>
  </div>
</div>
