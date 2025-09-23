<script lang="ts">
  import Modal from "./Modal.svelte";
  import RoundButton from "./RoundButton.svelte";

  const { 
    open = false, 
    title = "确认操作", 
    message = "确认要执行此操作吗？",
    confirmText = "确认",
    cancelText = "取消",
    isLoading = false,
    confirmButtonStyle = "danger", // "danger" | "accent"
    autoCloseOnConfirm = true, // 确认后是否自动关闭
    onClose = () => {},
    onConfirm = () => {},
    onCancel = () => {}
  } = $props<{
    open?: boolean;
    title?: string;
    message?: string;
    confirmText?: string;
    cancelText?: string;
    isLoading?: boolean;
    confirmButtonStyle?: "danger" | "accent";
    autoCloseOnConfirm?: boolean;
    onClose?: () => void;
    onConfirm?: () => void;
    onCancel?: () => void;
  }>();

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

  // 根据按钮样式设置颜色
  const getConfirmButtonColors = (style: string) => {
    switch (style) {
      case "danger":
        return {
          bgColor: "bg-error",
          textColor: "text-base-100",
          hoverColor: "hover:bg-error/90"
        };
      case "accent":
        return {
          bgColor: "bg-primary",
          textColor: "text-primary-content",
          hoverColor: "hover:bg-primary/90"
        };
      default:
        return {
          bgColor: "bg-primary",
          textColor: "text-primary-content",
          hoverColor: "hover:bg-primary/90"
        };
    }
  };

  const confirmColors = getConfirmButtonColors(confirmButtonStyle);
</script>

<Modal bind:this={modalRef} {open} onClose={handleModalClose} showCloseButton={false}>
  <div class="max-w-md flex flex-col">
    <!-- 头部 -->
    <div class="flex items-center justify-center px-6 pt-4 pb-0">
      <h2 class="text-sm">{title}</h2>
    </div>

    <!-- 内容 -->
    <div class="px-6 py-2 text-center text-base-content text-[12px]">
      {@html message}
    </div>

    <!-- 底部按钮 -->
    <div class="flex items-center justify-center gap-4 px-6 pt-2 pb-4">
      <RoundButton
        customClass="w-22"
        label={cancelText}
        size="h-8"
        fontSize="text-sm"
        bgColor="bg-base-200"
        textColor="text-base-content/80"
        hoverColor="hover:bg-base-300"
        onclick={handleCancel}
      />
      <RoundButton
        customClass="w-22"
        size="h-8"
        fontSize="text-sm"
        label={isLoading ? '处理中...' : confirmText}
        bgColor={confirmColors.bgColor}
        textColor={confirmColors.textColor}
        hoverColor={confirmColors.hoverColor}
        disabled={isLoading}
        onclick={handleConfirm}
      />
    </div>
  </div>
</Modal>
