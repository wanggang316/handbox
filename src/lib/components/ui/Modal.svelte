<script lang="ts">
  import { Dialog } from "bits-ui";
  import TrafficLightsRedButton from "./TrafficLightsRedButton.svelte";
  import TitleBar from "./TitleBar.svelte";

  // 对外 API 与旧版逐字段保持一致（drop-in）：11 个调用方零改动。内部改用 bits-ui
  // Dialog —— 白拿 Portal(到 <body>)、focus-trap、Escape、scroll-lock，并删除旧版
  // 为“圆角 overflow-hidden 会裁掉内部下拉”而把 children 渲染两遍的 hack（那会让子内容
  // 的 effect/表单状态翻倍）。z-index 走 app.css 的 --z-overlay/--z-modal token，不再裸值。
  interface Props {
    open?: boolean;
    title?: string;
    showCloseButton?: boolean;
    closeOnBackdropClick?: boolean;
    onClose?: () => void;
    children?: import("svelte").Snippet;
  }

  let {
    open = $bindable(false),
    title = "",
    showCloseButton = true,
    closeOnBackdropClick = false,
    onClose = () => {},
    children,
  }: Props = $props();

  // 单一关闭收敛点：任何关闭路径（Escape / 点击外部 / 红灯按钮 / 程序化置 false）都经
  // bits-ui 的 bind:open 把 open 变为 false，这里以 true→false 的迁移触发一次 onClose，
  // 取代旧版手写的 backdrop-click / Escape / 焦点三套逻辑（现由 Dialog.Content 提供）。
  let wasOpen = false;
  $effect(() => {
    if (wasOpen && !open) onClose();
    wasOpen = open;
  });

  // 保留旧版的公开方法（drop-in）：若干调用方经 `bind:this` 调 `modalRef.handleClose()`
  // 做程序化关闭。置 open=false 即触发 bits-ui 的退场动画并经上面的 $effect 回调 onClose。
  export function handleClose() {
    open = false;
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay
      class="dlg-overlay fixed inset-0"
      style="z-index: var(--z-overlay); background-color: var(--overlay);"
    />

    <!-- 窗口拖拽区：**仅在 open 时渲染**。bits-ui 的 Portal 会 eagerly mount 其“裸子节点”
         （Overlay/Content 各自有 presence 门控，裸节点没有），故若不加 {#if open}，每个
         “已挂载但关闭”的 Modal 都会把这条 `fixed; top:0; height:50px; z-9999` 顶部拖拽条
         注入 <body>——含多个关闭态 Modal 的页面（agents 3 个 / jobs 3 个）顶部工具栏会被
         数条拖拽条盖住而“打不开”。置于 overlay 之上、content 之下（10055 介于
         --z-overlay/--z-modal），使 modal 打开时顶部仍可拖动窗口（backdrop 会盖住根布局
         的 TitleBar）。 -->
    {#if open}
      <div style="position: relative; z-index: 10055;">
        <TitleBar showToggleButton={false} />
      </div>
    {/if}

    <!-- 居中用 `transform: translate(-50%,-50%)`（内联），**不用** Tailwind 的
         `-translate-x/y-1/2`：后者在 Tailwind v4 里写的是独立的 `translate:` 属性，会与
         下面 keyframe 动的 `transform:` 属性**叠加**，导致开场动画期间双倍偏移、动画结束
         （open 态非 forwards）transform 归零后又“跳”回中心。让居中与动画用同一个
         `transform` 属性即可消除跳变：开场/退场 keyframe 也都基于 translate(-50%,…)。 -->
    <Dialog.Content
      interactOutsideBehavior={closeOnBackdropClick ? "close" : "ignore"}
      class="dlg-content fixed left-1/2 top-1/2 max-h-[90vh] max-w-[90vw] rounded-xl border border-[var(--hairline)] bg-[var(--bg-card)] shadow-2xl outline-none"
      style="z-index: var(--z-modal); transform: translate(-50%, -50%);"
    >
      <!-- macOS 风红灯关闭 + 标题（保持原 Linear overlay 观感）。红灯经 open=false 关闭。 -->
      {#if showCloseButton || title}
        <div class="absolute left-0 top-0 z-10 flex items-center px-5 py-4">
          {#if showCloseButton}
            <TrafficLightsRedButton onClick={() => (open = false)} />
          {/if}
          <Dialog.Title
            class={title
              ? "ml-4 text-base font-medium text-base-content/80"
              : "sr-only"}
          >
            {title || "对话框"}
          </Dialog.Title>
        </div>
      {:else}
        <Dialog.Title class="sr-only">对话框</Dialog.Title>
      {/if}

      <!-- 内容只渲染一次（删除旧版 invisible 占位 + visible 覆盖的双渲染 hack）；
           不加 overflow-hidden —— 保留“内部下拉/Select 不被裁切”的既有行为。 -->
      {#if children}
        {@render children()}
      {/if}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
