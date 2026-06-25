<!--
  Quick Action 浮层的 Raycast 式统一面板（输入壳）。

  一张面板,自上而下:输入行(前导 sparkle 图标 + 可选 Agent scope chip + 大号输入)
  → 分隔线 → 内容区(Agent 列表 / transcript / 空态,经 `children` snippet 由父级注入)
  → footer(随模式切换的键位提示)。无模型选择、无 New(+)、无附件、无工具菜单。

  本组件只负责输入与键盘交互的**呈现与语义化**,不接 chat / 不建会话 / 不发消息——
  所有行为通过语义化回调交给父级(/quick/+page.svelte):
  - onSubmit   ↵(非空、非 IME 合成)→ 选择高亮 Agent(选择步)或发送消息(消息步)
  - onContinue ⌘↵ → 在对话中继续(answered 步)
  - onArrowUp / onArrowDown → 在 Agent 列表中上下移动高亮(选择步)
  - onDeselect Backspace(空输入)→ 取消已选 Agent,回到选择步

  键盘:IME 合成期间所有键交给输入法,不触发任何回调;Shift+Enter → 换行。
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { Sparkles, X } from "@lucide/svelte";
  import { t } from "$lib/i18n";

  interface Props {
    value: string;
    placeholder?: string;
    /** 已选 Agent 名;非空 → 显示 scope chip 并切到消息模式 footer。 */
    selectedAgentName?: string | null;
    /** 输入禁用(answered:一回合已发送,不再可输入)。 */
    disabled?: boolean;
    /** ⌘↵「在对话中继续」是否可用(已发送、有会话)。 */
    canContinue?: boolean;
    runError?: string | null;
    /** 是否有内容区(Agent 列表 / transcript / 空态);false 时面板仅输入行 + footer。 */
    hasContent?: boolean;
    /** 内容区,渲染在输入行与 footer 之间。 */
    children?: Snippet;
    onSubmit?: () => void;
    onContinue?: () => void;
    onArrowUp?: () => void;
    onArrowDown?: () => void;
    onDeselect?: () => void;
  }

  let {
    value = $bindable(""),
    placeholder,
    selectedAgentName = null,
    disabled = false,
    canContinue = false,
    runError = null,
    hasContent = false,
    children,
    onSubmit = () => {},
    onContinue = () => {},
    onArrowUp = () => {},
    onArrowDown = () => {},
    onDeselect = () => {},
  }: Props = $props();

  let textareaRef = $state<HTMLTextAreaElement | null>(null);
  let composing = $state(false);

  export function focus(): void {
    requestAnimationFrame(() => textareaRef?.focus());
  }

  function adjustTextareaHeight(): void {
    if (!textareaRef) return;
    textareaRef.style.height = "auto";
    const maxHeight = 132;
    textareaRef.style.height = Math.min(textareaRef.scrollHeight, maxHeight) + "px";
  }

  function handleInput(): void {
    adjustTextareaHeight();
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (composing || event.isComposing) return;

    // ⌘↵ / Ctrl+↵ → 在对话中继续。
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      if (canContinue) onContinue();
      return;
    }
    // ↵(非 Shift)→ 选择 / 发送(具体语义由父级按当前步骤决定)。
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      onSubmit();
      return;
    }
    // ↑↓ → 在 Agent 列表中移动高亮(选择步)。
    if (event.key === "ArrowDown") {
      event.preventDefault();
      onArrowDown();
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      onArrowUp();
      return;
    }
    // Backspace(空输入且已选 Agent)→ 取消选择,回到选择步。
    if (event.key === "Backspace" && value.length === 0 && selectedAgentName) {
      event.preventDefault();
      onDeselect();
      return;
    }
  }
</script>

<div
  class="quick-panel flex h-fit max-h-full w-full flex-col self-start overflow-hidden rounded-[14px] border border-white/10 text-[var(--base-content)] shadow-2xl ring-1 ring-black/5"
>
  <!-- 输入行 -->
  <div class="flex shrink-0 items-center gap-2.5 px-4">
    <Sparkles size={20} class="shrink-0 text-[var(--base-content)]/35" />
    {#if selectedAgentName}
      <span class="qa-chip">
        <span class="max-w-[140px] truncate">{selectedAgentName}</span>
        {#if !disabled}
          <button
            type="button"
            class="qa-chip-x"
            aria-label={t("common.cancel")}
            onclick={() => onDeselect()}
          >
            <X size={11} />
          </button>
        {/if}
      </span>
    {/if}
    <textarea
      bind:this={textareaRef}
      bind:value
      onkeydown={handleKeydown}
      oninput={handleInput}
      oncompositionstart={() => (composing = true)}
      oncompositionend={() => (composing = false)}
      {placeholder}
      {disabled}
      rows={1}
      class="composer-input w-full resize-none bg-transparent py-[14px] text-[15px] leading-6 text-[var(--base-content)] placeholder:text-[var(--base-content)]/35 focus:outline-none overflow-y-auto disabled:cursor-default"
    ></textarea>
  </div>

  <!-- 内容区(Agent 列表 / transcript / 空态) -->
  {#if hasContent}
    <div class="h-px w-full shrink-0 bg-[var(--hairline)]"></div>
    <div class="min-h-0 flex-1 overflow-y-auto">
      {@render children?.()}
    </div>
  {/if}

  {#if runError}
    <div class="shrink-0 px-4 pb-1.5 text-xs text-warning">{runError}</div>
  {/if}

  <!-- footer:键位提示随当前步骤切换。 -->
  <div
    class="flex h-11 shrink-0 items-center justify-end gap-1 border-t border-[var(--hairline)] bg-[var(--base-200)]/40 px-2.5"
  >
    {#if selectedAgentName}
      <!-- 消息步 / answered 步 -->
      {#if canContinue}
        <button type="button" onclick={() => onContinue()} class="qa-action">
          <kbd class="qa-key">⌘↵</kbd>
          <span>{t("quickaction.continueInChat")}</span>
        </button>
      {:else}
        <button
          type="button"
          onclick={() => onSubmit()}
          class="qa-action qa-action-primary"
        >
          <kbd class="qa-key">↵</kbd>
          <span>{t("quickaction.send")}</span>
        </button>
      {/if}
    {:else}
      <!-- 选择步 -->
      <span class="qa-action">
        <kbd class="qa-key">↑↓</kbd>
        <span>{t("quickaction.navigate")}</span>
      </span>
      <span class="qa-action qa-action-primary">
        <kbd class="qa-key">↵</kbd>
        <span>{t("quickaction.select")}</span>
      </span>
    {/if}
  </div>
</div>

<style>
  /* Raycast 式磨砂背景:透明窗口下用半透明 + backdrop blur 营造层次(原生 vibrancy
     若由窗口 effects 提供则叠加更佳)。 */
  .quick-panel {
    background: color-mix(in srgb, var(--bg-card) 60%, transparent);
    backdrop-filter: saturate(180%);
    -webkit-backdrop-filter: saturate(180%);
  }

  /* 已选 Agent 的 scope chip(Raycast 式作用域令牌)。 */
  .qa-chip {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    flex-shrink: 0;
    border-radius: 7px;
    padding: 0.2rem 0.4rem 0.2rem 0.55rem;
    font-size: 13px;
    line-height: 1;
    background: color-mix(in srgb, var(--primary) 16%, transparent);
    color: var(--base-content);
  }
  .qa-chip-x {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    padding: 0.1rem;
    color: color-mix(in srgb, var(--base-content) 55%, transparent);
    transition: background-color 0.12s ease;
  }
  .qa-chip-x:hover {
    background: color-mix(in srgb, var(--base-content) 12%, transparent);
    color: var(--base-content);
  }

  .qa-action {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    border-radius: 7px;
    padding: 0.25rem 0.5rem;
    font-size: 12px;
    color: color-mix(in srgb, var(--base-content) 62%, transparent);
    transition: background-color 0.12s ease;
  }
  button.qa-action:hover {
    background: color-mix(in srgb, var(--base-content) 8%, transparent);
  }
  .qa-action-primary {
    color: color-mix(in srgb, var(--base-content) 88%, transparent);
  }
  .qa-key {
    display: inline-flex;
    min-width: 1.1rem;
    height: 1.1rem;
    align-items: center;
    justify-content: center;
    border-radius: 5px;
    padding: 0 0.25rem;
    font-size: 11px;
    font-family: inherit;
    background: color-mix(in srgb, var(--base-content) 10%, transparent);
    color: color-mix(in srgb, var(--base-content) 70%, transparent);
  }
</style>
