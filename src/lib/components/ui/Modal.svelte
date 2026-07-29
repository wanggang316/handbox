<script lang="ts">
  import { Dialog } from "bits-ui";
  import TrafficLightsRedButton from "./TrafficLightsRedButton.svelte";
  import TitleBar from "./TitleBar.svelte";

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

  // 所有关闭路径（Escape / 点击外部 / 红灯 / 程序化）都经 bind:open 收敛为 open=false；
  // 捕捉 true→false 迁移触发一次 onClose。
  let wasOpen = false;
  $effect(() => {
    if (wasOpen && !open) onClose();
    wasOpen = open;
  });

  // 供调用方经 bind:this 程序化关闭。
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

    <!-- bits-ui Portal 会立即把裸子节点 mount 到 <body>（不做 open 门控，只有
         Overlay/Content 自带 presence），故 TitleBar 必须 {#if open}——否则关闭态的
         Modal 也会往 <body> 注入这条 fixed 顶部拖拽条。打开时它位于 backdrop 之上
         （z 介于 --z-overlay/--z-modal），使 modal 打开时顶部仍可拖动窗口。 -->
    {#if open}
      <div style="position: relative; z-index: 10055;">
        <TitleBar showToggleButton={false} />
      </div>
    {/if}

    <!-- 居中与入场/退场动画统一用 transform：不用 Tailwind 的 -translate-x/y-1/2，
         因其在 Tailwind v4 下写的是独立的 translate 属性，会与 keyframe 的 transform 叠加。 -->
    <!-- 打开时不 autofocus 首个可聚焦元素：无鼠标交互在前时（如启动即弹的升级框），
         autofocus 会被判定为 :focus-visible，首个按钮凭空带上焦点环。 -->
    <Dialog.Content
      interactOutsideBehavior={closeOnBackdropClick ? "close" : "ignore"}
      onOpenAutoFocus={(e) => e.preventDefault()}
      class="dlg-content fixed left-1/2 top-1/2 max-h-[90vh] max-w-[90vw] rounded-xl border border-[var(--hairline)] bg-[var(--bg-card)] shadow-2xl outline-none"
      style="z-index: var(--z-modal); transform: translate(-50%, -50%);"
    >
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

      <!-- 不加 overflow-hidden：避免裁切内部下拉 / Select。 -->
      {#if children}
        {@render children()}
      {/if}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
