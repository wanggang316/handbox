<script lang="ts">
  import Modal from "./Modal.svelte";
  import RoundButton from "./RoundButton.svelte";
  import { t } from "$lib/i18n";

  // 定义操作按钮类型
  type ActionButton = {
    label: string;
    style?: "danger" | "accent" | "primary" | "secondary";
    onClick: () => void;
  };

  const {
    open = false,
    title,
    message,
    confirmText,
    cancelText,
    isLoading = false,
    confirmButtonStyle = "danger", // "danger" | "accent"
    autoCloseOnConfirm = true, // 确认后是否自动关闭
    actions = undefined, // 多个操作按钮配置（如果提供，将忽略 confirmText/cancelText）
    onClose = () => {},
    onConfirm = () => {},
    onCancel = () => {},
  } = $props<{
    open?: boolean;
    title?: string;
    message?: string;
    confirmText?: string;
    cancelText?: string;
    isLoading?: boolean;
    confirmButtonStyle?: "danger" | "accent" | "primary";
    autoCloseOnConfirm?: boolean;
    actions?: ActionButton[]; // 支持多个自定义操作按钮
    onClose?: () => void;
    onConfirm?: () => void;
    onCancel?: () => void;
  }>();

  // 文案回退：未传入时用本地化默认值（保持语言切换时响应式）
  const resolvedTitle = $derived(title ?? t("ui.confirmTitle"));
  const resolvedMessage = $derived(message ?? t("ui.confirmMessage"));
  const resolvedConfirmText = $derived(confirmText ?? t("common.confirm"));
  const resolvedCancelText = $derived(cancelText ?? t("common.cancel"));

  let modalRef: Modal;

  // 暴露 modalRef 供外部访问
  export { modalRef };

  function handleConfirm() {
    onConfirm();
    // 根据配置决定是否自动关闭
    if (autoCloseOnConfirm) {
      modalRef?.handleClose();
    }
  }

  function handleCancel() {
    // 取消时先调用回调，再触发关闭动画
    onCancel();
    modalRef?.handleClose();
  }

  function handleModalClose() {
    // 动画完成后调用 onClose 回调
    onClose();
  }

  // 处理自定义操作按钮点击
  function handleActionClick(action: ActionButton) {
    action.onClick();
    if (autoCloseOnConfirm) {
      modalRef?.handleClose();
    }
  }
</script>

<Modal
  bind:this={modalRef}
  {open}
  onClose={handleModalClose}
  showCloseButton={false}
>
  <div class="max-w-md flex flex-col">
    <!-- 头部 -->
    <div class="flex items-center justify-center px-6 pt-4 pb-0">
      <h2 class="text-sm">{resolvedTitle}</h2>
    </div>

    <!-- 内容 -->
    <div class="px-6 py-2 text-center text-base-content text-[12px]">
      {@html resolvedMessage}
    </div>

    <!-- 底部按钮 -->
    {#if actions && actions.length > 0}
      <!-- 多个操作按钮：垂直排列 -->
      <div class="flex flex-col gap-2 px-6 pt-2 pb-4">
        {#each actions as action}
          <RoundButton
            customClass="w-full"
            label={action.label}
            size="h-9"
            fontSize="text-sm"
            variant={action.style ?? "primary"}
            disabled={isLoading}
            onclick={() => handleActionClick(action)}
          />
        {/each}
      </div>
    {:else}
      <!-- 默认两个按钮：水平排列 -->
      <div class="flex items-center justify-center gap-4 px-6 pt-2 pb-4">
        <RoundButton
          customClass="w-22"
          label={resolvedCancelText}
          size="h-8"
          fontSize="text-sm"
          variant="secondary"
          onclick={handleCancel}
        />
        <RoundButton
          customClass="w-22"
          size="h-8"
          fontSize="text-sm"
          label={isLoading ? t("ui.processing") : resolvedConfirmText}
          variant={confirmButtonStyle}
          disabled={isLoading}
          onclick={handleConfirm}
        />
      </div>
    {/if}
  </div>
</Modal>
