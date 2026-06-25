<!--
  Quick Action 浮层 composer 宿主页。

  一张铺满 frameless / transparent NSPanel 的圆角主题卡片，内含 QuickInput
  composer。Esc 调用 `quick_action_hide` 隐藏浮层。

  关键：NSPanel 隐藏窗口而非销毁 webview，故除 onMount 外，还需在窗口重新获得焦点
  时再次聚焦输入框，确保每次召唤都能立即键入（VAL-OVERLAY-007）。

  范围说明：本页持有 composer 的状态（value / model / running）并提供回调。
  当前回调是 stub（console.log）；后续 feature（qa-session-send-stream）把它们
  换成真实的 session 创建 / runAgentStream / stop / continue-in-chat 逻辑，
  无需改动 QuickInput 的形状。
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import QuickInput from "$lib/components/quickaction/QuickInput.svelte";
  import type { ModelWithProvider } from "$lib/types/provider";
  import { isTauriEnvironment } from "$lib/utils/tauri";

  let composer = $state<QuickInput | null>(null);

  // composer 本地状态（父级拥有，回调消费）。
  let value = $state("");
  let running = $state(false);
  let selectedModel = $state<ModelWithProvider | null>(null);

  // ⌘↵ continue-in-chat 仅在有内容时可用（后续接会话后由真实条件决定）。
  const canContinue = $derived(value.trim().length > 0);

  /** 聚焦输入框（下一帧，确保窗口/DOM 就绪后才聚焦）。 */
  function focusInput(): void {
    composer?.focus();
  }

  // ── Stub 回调（后续 feature 替换为真实 session/run 逻辑）─────────────────
  function handleSend(text: string): void {
    console.log("quick:send", text);
  }

  function handleContinue(): void {
    console.log("quick:continue", value);
  }

  function handleStop(): void {
    console.log("quick:stop");
  }

  function handleNewClear(): void {
    console.log("quick:newClear");
    value = "";
    focusInput();
  }

  function handleModelSelect(model: ModelWithProvider): void {
    selectedModel = model;
    console.log("quick:modelSelect", model.id);
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

<svelte:window onkeydown={handleKeydown} />

<QuickInput
  bind:this={composer}
  bind:value
  {running}
  {selectedModel}
  {canContinue}
  onModelSelect={handleModelSelect}
  onSend={handleSend}
  onContinue={handleContinue}
  onStop={handleStop}
  onNewClear={handleNewClear}
/>

<style>
  /* 透明窗口：让 body 背景透出，仅卡片可见，保持 frameless 圆角浮层观感。 */
  :global(html),
  :global(body) {
    background: transparent;
  }
</style>
