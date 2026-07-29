<!--
  Raycast-style panel shell for the Quick Action overlay: input row (sparkle icon
  + optional agent scope chip + textarea) → divider → content area (injected via
  `children`) → footer with step-dependent key hints.

  Presentation only — no chat, sessions, or sending. Behavior is delegated to the
  parent through semantic callbacks: onSubmit (↵), onContinue (⌘↵), onArrowUp/
  Down (list highlight), onDeselect (Backspace on empty input). During IME
  composition every key goes to the input method; Shift+Enter inserts a newline.
-->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { Sparkles, X } from "@lucide/svelte";
  import { t } from "$lib/i18n";

  interface Props {
    value: string;
    placeholder?: string;
    /** Selected agent name; non-null shows the scope chip and the message-step footer. */
    selectedAgentName?: string | null;
    /** Disabled once the single allowed turn has been sent (answered step). */
    disabled?: boolean;
    /** Whether ⌘↵ "continue in chat" is available (message sent, session exists). */
    canContinue?: boolean;
    runError?: string | null;
    /** When false the panel renders only the input row + footer. */
    hasContent?: boolean;
    /** Content area rendered between the input row and the footer. */
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

    // ⌘↵ / Ctrl+↵ → continue in chat.
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      if (canContinue) onContinue();
      return;
    }
    // Plain ↵ → select / send (semantics decided by the parent per step).
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      onSubmit();
      return;
    }
    // ↑↓ → move the agent-list highlight (selection step).
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
    // Backspace on empty input with an agent selected → deselect, back to selection step.
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

  {#if hasContent}
    <div class="h-px w-full shrink-0 bg-[var(--hairline)]"></div>
    <div class="min-h-0 flex-1 overflow-y-auto">
      {@render children?.()}
    </div>
  {/if}

  {#if runError}
    <div class="shrink-0 px-4 pb-1.5 text-xs text-warning">{runError}</div>
  {/if}

  <div
    class="flex h-11 shrink-0 items-center justify-end gap-1 border-t border-[var(--hairline)] bg-[var(--base-200)]/40 px-2.5"
  >
    {#if selectedAgentName}
      <!-- message / answered step -->
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
      <!-- selection step -->
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
  /* Raycast-style frosted background: translucency + backdrop blur over the
     transparent window (native vibrancy from window effects stacks on top). */
  .quick-panel {
    background: color-mix(in srgb, var(--bg-card) 60%, transparent);
    backdrop-filter: saturate(180%);
    -webkit-backdrop-filter: saturate(180%);
  }

  /* Scope chip for the selected agent (Raycast-style scope token). */
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
    transition: background-color var(--dur-fast) ease;
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
    transition: background-color var(--dur-fast) ease;
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
