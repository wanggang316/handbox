<!--
  Quick Action 浮层的轻量 composer。

  AgentInput 的精简版：textarea + 模型选择按钮 + 发送/停止 + 新建/清空。
  无附件、无 slash/skill 浮层、无内置工具菜单、无思考强度控制。

  本组件只负责输入与键盘交互，不接 agent / 不起 run / 不做会话创建——
  所有行为通过 props 上的回调交给父级（/quick/+page.svelte）。后续 feature
  （qa-session-send-stream）把这些 stub 回调换成真实的 session/run 逻辑，
  无需改动本组件的形状。

  键盘（镜像 AgentInput ~382-470 的 send/steer 键处理，但精简）：
  - Enter（非空）→ onSend(text)；Shift+Enter → 换行
  - ⌘↵ (Cmd+Enter) → onContinue()（continue-in-chat；此处仅回调，主窗口交接是后续里程碑）
  - running 为 true 时发送控件变为 Stop → onStop()
  - IME 合成期间所有键交给输入法，不触发发送/继续（VAL-SLASH-014 同理）
-->
<script lang="ts">
  import { ArrowUp, Square, Plus } from "@lucide/svelte";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import ChatModelSelectButton from "$lib/components/chat/ChatModelSelectButton.svelte";
  import { t } from "$lib/i18n";
  import type { ModelWithProvider } from "$lib/types/provider";

  interface Props {
    /** 输入文本（双向绑定，文本即唯一真源）。 */
    value: string;
    /** 是否有活跃 run —— 驱动 Send <-> Stop 切换（父级决定）。 */
    running?: boolean;
    /**
     * 是否有待审批的危险工具调用 —— 对话暂停在审批弹窗上。镜像 AgentInput：
     * 置位时禁用输入与发送（占位文案改为审批提示），发送为干净 no-op，直到用户
     * 在弹窗里允许 / 拒绝（VAL-COMMS-016）。
     */
    awaitingApproval?: boolean;
    /**
     * run 启动失败的错误提示（父级在 runAgentStream 同步拒绝时置位）。镜像
     * AgentInput 的 modelPrompt：非空时在控件上方以 warning 色展示一行，便于重试
     * （此时输入已由父级回填，VAL-COMMS-018）。
     */
    runError?: string | null;
    /** 当前选中模型，透传给 ChatModelSelectButton。 */
    selectedModel?: ModelWithProvider | null;
    /** 模型选择回调，透传给 ChatModelSelectButton。 */
    onModelSelect?: (model: ModelWithProvider) => void;
    /** ⌘↵ continue-in-chat 是否可用（父级按有无会话/内容决定）。 */
    canContinue?: boolean;
    /** textarea placeholder。 */
    placeholder?: string;
    /** Enter（非空）发送：把当前文本交给父级。 */
    onSend?: (text: string) => void;
    /** ⌘↵ continue-in-chat。 */
    onContinue?: () => void;
    /** running 时点击 Stop。 */
    onStop?: () => void;
    /** 新建/清空控件。 */
    onNewClear?: () => void;
  }

  let {
    value = $bindable(""),
    running = false,
    awaitingApproval = false,
    runError = null,
    selectedModel = null,
    onModelSelect = () => {},
    canContinue = false,
    placeholder,
    onSend = () => {},
    onContinue = () => {},
    onStop = () => {},
    onNewClear = () => {},
  }: Props = $props();

  let textareaRef = $state<HTMLTextAreaElement | null>(null);

  // IME 合成标记：合成期间 Enter / ⌘↵ 不触发发送/继续，交给输入法。
  let composing = $state(false);

  /** 暴露聚焦给父级（每次召唤浮层都要重新聚焦输入框）。 */
  export function focus(): void {
    requestAnimationFrame(() => textareaRef?.focus());
  }

  function adjustTextareaHeight(): void {
    if (!textareaRef) return;
    textareaRef.style.height = "auto";
    const maxHeight = 200;
    textareaRef.style.height = Math.min(textareaRef.scrollHeight, maxHeight) + "px";
  }

  function handleInput(): void {
    adjustTextareaHeight();
  }

  // 发送：纯空白为干净 no-op（不回调）；running 时不在此处理（按钮变 Stop）；
  // 待审批暂停时干净 no-op（对话挂起在审批弹窗上，VAL-COMMS-016）。
  function send(): void {
    if (awaitingApproval) return;
    if (running) return;
    if (!value.trim()) return;
    onSend(value);
  }

  // Enter 发送；Shift+Enter 换行；⌘↵（Cmd/Ctrl+Enter）= continue-in-chat。
  // 镜像 AgentInput 的键处理，但去掉 slash 浮层分支。
  function handleKeydown(event: KeyboardEvent): void {
    // IME 合成中：所有键交给输入法（双保险：标记 + isComposing）。
    if (composing || event.isComposing) return;

    if (event.key !== "Enter") return;

    // ⌘↵ / Ctrl+↵ → continue-in-chat（仅在父级允许时）。
    if (event.metaKey || event.ctrlKey) {
      event.preventDefault();
      if (canContinue) onContinue();
      return;
    }

    // Shift+Enter → 换行（默认行为，不拦截）。
    if (event.shiftKey) return;

    // Enter → 发送。
    event.preventDefault();
    send();
  }

  function handleCompositionStart(): void {
    composing = true;
  }

  function handleCompositionEnd(): void {
    composing = false;
  }
</script>

<div
  class="flex h-full w-full flex-col rounded-xl border border-[var(--hairline)] bg-[var(--bg-card)] text-[var(--base-content)] shadow-2xl overflow-hidden"
>
  <textarea
    bind:this={textareaRef}
    bind:value
    onkeydown={handleKeydown}
    oninput={handleInput}
    oncompositionstart={handleCompositionStart}
    oncompositionend={handleCompositionEnd}
    placeholder={awaitingApproval
      ? t("agent.input.awaitingApprovalPlaceholder")
      : (placeholder ?? t("quickaction.placeholder"))}
    disabled={awaitingApproval}
    rows={1}
    class="flex-1 w-full resize-none bg-transparent px-4 py-3 text-sm leading-relaxed text-[var(--base-content)] placeholder:text-[var(--base-content)]/40 focus:outline-none max-h-[200px] overflow-y-auto disabled:cursor-not-allowed disabled:opacity-60"
  ></textarea>

  {#if runError}
    <div class="px-4 pb-1 text-xs text-warning">
      {runError}
    </div>
  {/if}

  <div class="flex flex-row items-center justify-between gap-3 px-3 pb-2 pt-0">
    <IconButton
      icon={Plus}
      ariaLabel={t("quickaction.newClear")}
      title={t("quickaction.newClear")}
      onclick={() => onNewClear()}
    />

    <div class="flex flex-row items-center gap-3">
      <ChatModelSelectButton {selectedModel} {onModelSelect} />
      {#if running}
        <CircleButton
          icon={Square}
          iconSize={16}
          size="w-8 h-8"
          ariaLabel={t("quickaction.stop")}
          onclick={() => onStop()}
        />
      {:else}
        <CircleButton
          icon={ArrowUp}
          iconSize={18}
          size="w-8 h-8"
          ariaLabel={t("quickaction.send")}
          disabled={awaitingApproval}
          onclick={send}
        />
      {/if}
    </div>
  </div>
</div>
