<!--
  Quick Action 浮层 input shell。

  一张铺满 frameless / transparent NSPanel 的圆角主题卡片，内含单个自动聚焦的
  textarea。Esc 调用 `quick_action_hide` 隐藏浮层。

  关键：NSPanel 隐藏窗口而非销毁 webview，故除 onMount 外，还需在窗口重新获得焦点
  时再次聚焦输入框，确保每次召唤都能立即键入（VAL-OVERLAY-007）。

  范围说明：这里只是 input shell——不接 agent、不放 model picker / send 按钮，
  真正的 composer 由后续 agent-comms feature 替换/增强。
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { t } from "$lib/i18n";
  import { isTauriEnvironment } from "$lib/utils/tauri";

  let inputEl = $state<HTMLTextAreaElement | null>(null);
  let value = $state("");

  /** 聚焦输入框（下一帧，确保窗口/DOM 就绪后才聚焦）。 */
  function focusInput(): void {
    requestAnimationFrame(() => inputEl?.focus());
  }

  onMount(() => {
    // 首次挂载即聚焦（首次召唤）。
    focusInput();

    if (!isTauriEnvironment()) {
      // 纯浏览器预览：无 Tauri 窗口事件，挂载聚焦已足够验证渲染/聚焦/键入。
      return;
    }

    // webview 跨 hide/show 存活：每次窗口重新获得焦点都重新聚焦，
    // 使 VAL-OVERLAY-007 在每次召唤（而非仅首次）都成立。
    let unlisten: UnlistenFn | null = null;
    getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (focused) focusInput();
      })
      .then((fn) => {
        unlisten = fn;
      });

    return () => unlisten?.();
  });

  /** Esc → 隐藏浮层（仅在 Tauri 环境可解析该命令）。 */
  async function handleKeydown(event: KeyboardEvent): Promise<void> {
    if (event.key !== "Escape") return;
    event.preventDefault();
    if (!isTauriEnvironment()) return;
    await invoke("quick_action_hide");
  }
</script>

<div
  class="flex h-full w-full flex-col rounded-xl border border-[var(--hairline)] bg-[var(--bg-card)] text-[var(--base-content)] shadow-2xl overflow-hidden"
>
  <textarea
    bind:this={inputEl}
    bind:value
    onkeydown={handleKeydown}
    placeholder={t("quickaction.placeholder")}
    rows={1}
    class="flex-1 w-full resize-none bg-transparent px-4 py-3 text-sm leading-relaxed text-[var(--base-content)] placeholder:text-[var(--base-content)]/40 focus:outline-none"
  ></textarea>
</div>

<style>
  /* 透明窗口：让 body 背景透出，仅卡片可见，保持 frameless 圆角浮层观感。 */
  :global(html),
  :global(body) {
    background: transparent;
  }
</style>
