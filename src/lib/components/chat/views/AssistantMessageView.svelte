<script lang="ts">
  import {
    Copy,
    RotateCcw,
    Trash2,
    ChevronDown,
    ChevronRight,
  } from "lucide-svelte";
  import type { Message } from "$lib/types";
  import { messageStore, mcpState } from "$lib/states";
  import { openInBrowser, renderMarkdown } from "$lib/utils";

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

  // 获取工具调用数据
  const toolCalls = $derived(() => message?.toolCalls || []);

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

  // 检查是否有任何工具调用正在执行
  const isExecuting = $derived(() => {
    const calls = toolCalls();
    // 检查是否有任何工具调用的状态为 executing
    return calls.some(call => call.executionStatus === 'executing');
  });

  // 检查是否有工具调用需要手动执行
  const needsManualExecution = $derived(() => {
    const calls = toolCalls();
    if (calls.length === 0) return false;

    // 直接从工具调用的 executionMode 字段判断
    return calls.some(call => call.executionMode === 'manual');
  });

  // 获取工具调用的显示状态
  function getToolExecutionStatusDisplay(status?: string) {
    switch (status) {
      case 'pending':
        return { text: '待执行', icon: '⏸️', color: 'text-gray-600' };
      case 'executing':
        return { text: '执行中', icon: '⌛️', color: 'text-blue-600' };
      case 'completed':
        return { text: '完成', icon: '✅', color: 'text-green-600' };
      case 'failed':
        return { text: '失败', icon: '❌', color: 'text-red-600' };
      default:
        return { text: '未知', icon: '❓', color: 'text-gray-400' };
    }
  }

  // 自动执行工具调用
  // 当消息有自动执行模式的工具调用且状态为 pending 时，自动触发执行
  $effect(() => {
    const calls = toolCalls();
    if (!message?.id || calls.length === 0 || isStreaming) return;

    // 检查是否有需要自动执行的工具调用
    const autoExecuteCalls = calls.filter(
      call => call.executionMode === 'auto' && call.executionStatus === 'pending'
    );

    if (autoExecuteCalls.length > 0) {
      // 延迟一小段时间后执行，确保消息已经完全渲染
      setTimeout(() => {
        handleExecuteToolCalls();
      }, 100);
    }
  });

  async function handleExecuteToolCalls() {
    const calls = toolCalls();
    if (calls.length === 0) {
      console.warn('没有找到工具调用');
      return;
    }

    if (!message?.id) {
      console.error('消息 ID 不存在');
      return;
    }

    try {
      console.log('执行工具调用:', calls);

      // 如果需要手动执行，先删除后续消息
      if (needsManualExecution()) {
        console.log('手动工具执行：删除后续消息');
        // 这里应该调用删除后续消息的 API
        // 但目前我们直接执行工具
      }

      // 使用消息状态管理来执行工具调用
      await messageStore.executeAllToolCalls(message.id, calls);

    } catch (error) {
      console.error('执行工具调用失败:', error);
    }
  }

  function closestButton(target: EventTarget | null): HTMLButtonElement | null {
    if (!(target instanceof Element)) return null;
    return target.closest<HTMLButtonElement>(".markdown-code-block__copy");
  }

  function closestLink(target: EventTarget | null): HTMLAnchorElement | null {
    if (!(target instanceof Element)) return null;
    return target.closest<HTMLAnchorElement>("a[href]");
  }

  function isExternalLink(link: HTMLAnchorElement): boolean {
    const href = link.getAttribute("href")?.trim();
    if (!href) return false;

    const lowerHref = href.toLowerCase();
    if (lowerHref.startsWith("http://") || lowerHref.startsWith("https://")) {
      return true;
    }

    return lowerHref.startsWith("mailto:") || lowerHref.startsWith("tel:");
  }

  async function openMarkdownLink(link: HTMLAnchorElement) {
    try {
      await openInBrowser(link.href);
    } catch (error) {
      console.error("Failed to open markdown link", error);
    }
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

  function markdownInteractions(node: HTMLElement) {
    const handleClick = async (event: MouseEvent) => {
      const button = closestButton(event.target);
      if (button) {
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
        return;
      }

      const link = closestLink(event.target);
      if (link && isExternalLink(link)) {
        event.preventDefault();
        event.stopPropagation();
        await openMarkdownLink(link);
      }
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
        class="w-8 h-8 rounded-full bg-base-200 flex items-center justify-center"
      >
        <img
          src={providerConfig()?.icon}
          alt={providerConfig()?.type_name || "AI"}
          class="w-4 h-4 object-contain"
        />
      </div>

      {#if message?.config?.modelId}
        <div class="flex items-center gap-1 text-base-content/60 text-xs">
          {message.config.modelId}
        </div>
      {/if}
    </div>

    <!-- 消息内容 -->
    <div class="flex-1 min-w-0">
      {#if isMessageLoading}
        <!-- 加载状态 -->
        <div class="max-w-full py-2 text-base-content flex items-center">
          <div class="h-4 w-4 rounded-full bg-current animate-[pulse-scale_1.5s_ease-in-out_infinite]"></div>
        </div>
      {:else}
        <!-- 消息气泡 -->
        <div class="max-w-full py-0 text-base-content">
          <!-- 推理过程（如果有） -->
          {#if message?.reasoning}
            <div class="mb-4">
              <!-- 推理过程标题，可点击折叠 -->
              <button
                class="flex items-center gap-1 my-2 text-left hover:bg-base-300 rounded-full py-1 px-2"
                onclick={toggleReasoning}
              >
                {#if reasoningExpanded}
                  <ChevronDown size={16} class="text-base-content" />
                {:else}
                  <ChevronRight size={16} class="text-base-content" />
                {/if}
                <span class="text-sm font-medium text-base-content/80">
                  {isReasoning ? "推理中..." : "推理过程"}
                </span>
              </button>

              <!-- 推理过程内容，根据展开状态显示 -->
              {#if reasoningExpanded}
                <div
                  class="mt-2 mb-6 px-4 text-sm border-l border-base-300 text-base-content/80 break-words leading-relaxed reasoning-content markdown-content"
                  use:markdownInteractions
                >
                  {@html renderMarkdown(message.reasoning)}
                </div>
              {/if}
            </div>
          {/if}

          <!-- 消息内容 -->
          <div
            class="flex-1 break-words text-[15px] leading-[1.6] markdown-content"
            use:markdownInteractions
          >
            {@html renderMarkdown(message?.content || "")}
          </div>

          <!-- 工具调用记录 -->
          {#if toolCalls().length > 0}
            <div class="mb-4 rounded-lg border border-blue-400/40 bg-blue-50/80 p-3 text-sm text-blue-900 dark:bg-blue-900/40 dark:text-blue-100">
              <div class="mb-2 font-medium flex items-center gap-2">
                <span>工具调用</span>
                <span class="text-[10px] px-1.5 py-0.5 rounded bg-blue-500/20">
                  {toolCalls().length} 个工具
                </span>
              </div>
              <div class="space-y-2">
                {#each toolCalls() as tool}
                  {@const statusDisplay = getToolExecutionStatusDisplay(tool.executionStatus)}
                  <div class="rounded-md border border-blue-400/30 bg-white/80 dark:bg-blue-900/50 p-2 text-xs">
                    <!-- 工具信息头部 -->
                    <div class="flex items-center justify-between mb-2">
                      <div class="flex items-center gap-2">
                        <div class="font-semibold">{tool.function?.name || `工具 ${tool.index}`}</div>
                        <!-- 执行模式标签 -->
                        {#if tool.executionMode === 'manual'}
                          <span class="text-[10px] px-1.5 py-0.5 rounded bg-yellow-500/20 text-yellow-700 dark:text-yellow-300">
                            手动
                          </span>
                        {:else}
                          <span class="text-[10px] px-1.5 py-0.5 rounded bg-green-500/20 text-green-700 dark:text-green-300">
                            自动
                          </span>
                        {/if}
                      </div>
                      <!-- 执行状态和操作按钮 -->
                      <div class="flex items-center gap-2">
                        <span class="text-[10px] {statusDisplay.color} flex items-center gap-1">
                          <span>{statusDisplay.icon}</span>
                          <span>{statusDisplay.text}</span>
                        </span>

                        <!-- 手动执行或重新执行按钮 -->
                        {#if tool.executionMode === 'manual'}
                          {#if tool.executionStatus === 'pending'}
                            <button
                              class="px-2 py-0.5 text-[10px] bg-blue-600 hover:bg-blue-700 text-white rounded disabled:opacity-50"
                              onclick={() => handleExecuteToolCalls()}
                              disabled={isExecuting()}
                            >
                              执行
                            </button>
                          {:else if tool.executionStatus === 'failed' || tool.executionStatus === 'completed'}
                            <button
                              class="px-2 py-0.5 text-[10px] bg-gray-600 hover:bg-gray-700 text-white rounded disabled:opacity-50"
                              onclick={() => handleExecuteToolCalls()}
                              disabled={isExecuting()}
                            >
                              重新执行
                            </button>
                          {/if}
                        {/if}
                      </div>
                    </div>

                    <!-- 工具ID和类型 -->
                    <div class="text-gray-500 text-[10px] mb-2">
                      ID: {tool.id || 'N/A'} | 类型: {tool.toolType || 'function'}
                    </div>

                    <!-- 工具参数 -->
                    {#if tool.function?.arguments}
                      <div class="mt-2">
                        <div class="mb-1 text-[10px] text-gray-600 dark:text-gray-400 font-medium">参数:</div>
                        <pre class="max-h-32 overflow-auto rounded bg-base-200/70 dark:bg-black/30 p-2 text-[10px] text-base-content/80">
{tool.function.arguments}
                        </pre>
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            </div>
          {/if}

          {#if !isStreaming && !isMessageLoading}
          <!-- 性能信息 -->
          <div class="flex flex-row gap-2 mt-6 text-xs text-base-content/60">
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
                class="p-1.5 text-base-content/60 hover:text-base-content hover:bg-base-200 rounded transition-colors"
                title="复制消息"
                onclick={handleCopy}
              >
                <Copy class="w-3.5 h-3.5" />
              </button>

              <!-- 重新生成按钮 -->
              <button
                class="p-1.5 text-base-content/60 hover:text-base-content hover:bg-base-200 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
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
                class="p-1.5 text-base-content/60 hover:text-error hover:bg-error/10 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
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
