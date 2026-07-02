<script lang="ts">
  import Modal from "./Modal.svelte";
  import Button from "./Button.svelte";
  import { t } from "$lib/i18n";
  import type { Snippet } from "svelte";

  // 创建/编辑类弹窗的统一壳：header（Modal 红灯 + 标题）、底部操作栏
  // （左侧 hint/error，右侧 取消 + 主按钮）、宽度与间距，三个表单弹窗
  // （Agent / Job / MCP）共用，保证观感一致。
  interface Props {
    open?: boolean;
    title?: string;
    onClose?: () => void;
    /** md = 560px 单栏；lg = 920px 主区 + 右侧配置栏（aside snippet） */
    size?: "md" | "lg";
    saving?: boolean;
    submitLabel?: string;
    cancelLabel?: string;
    submitDisabled?: boolean;
    onSubmit?: () => void;
    /** 底部左侧常驻提示（如「保存后按计划自动运行」） */
    hint?: string;
    /** 保存错误：优先于 hint 显示 */
    error?: string | null;
    /** 主内容区 */
    children?: Snippet;
    /** 右侧配置栏（仅 size="lg" 生效） */
    aside?: Snippet;
  }

  let {
    open = $bindable(false),
    title = "",
    onClose = () => {},
    size = "md",
    saving = false,
    submitLabel = "",
    cancelLabel = "",
    submitDisabled = false,
    onSubmit = () => {},
    hint = "",
    error = null,
    children,
    aside,
  }: Props = $props();

  const width = $derived(size === "lg" ? "w-[920px]" : "w-[560px]");
</script>

<Modal bind:open {title} {onClose}>
  <div class="{width} max-w-[92vw] max-h-[86vh] flex flex-col">
    <!-- 主体：主内容区 + 可选右侧配置栏；pt 给 Modal 的红灯/标题行留位 -->
    <div class="flex flex-1 min-h-0 pt-13">
      <div class="flex-1 min-w-0 overflow-y-auto px-7 pb-6">
        {#if children}
          {@render children()}
        {/if}
      </div>
      {#if size === "lg" && aside}
        <div
          class="w-[300px] shrink-0 overflow-y-auto border-l border-[var(--hairline)] px-5 pb-6"
        >
          {@render aside()}
        </div>
      {/if}
    </div>

    <!-- 底部操作栏：左 hint/error，右 取消 + 主按钮 -->
    <div
      class="flex items-center justify-between gap-4 border-t border-[var(--hairline)] px-5 py-3"
    >
      <div class="min-w-0 flex-1 truncate text-xs {error ? 'text-error' : 'text-base-content/50'}">
        {error || hint}
      </div>
      <div class="flex shrink-0 items-center gap-2.5">
        <Button variant="ghost" onclick={onClose} disabled={saving}>
          {cancelLabel || t("common.cancel")}
        </Button>
        <Button
          variant="primary"
          onclick={onSubmit}
          disabled={saving || submitDisabled}
        >
          {submitLabel}
        </Button>
      </div>
    </div>
  </div>
</Modal>
