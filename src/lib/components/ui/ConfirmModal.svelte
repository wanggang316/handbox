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
          bgColor: "bg-red-600",
          textColor: "text-white",
          hoverColor: "hover:bg-red-700"
        };
      case "accent":
        return {
          bgColor: "bg-bg-accent",
          textColor: "text-white",
          hoverColor: "hover:bg-bg-accent-hover"
        };
      default:
        return {
          bgColor: "bg-bg-accent",
          textColor: "text-white",
          hoverColor: "hover:bg-bg-accent-hover"
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
    <div class="px-6 py-2 text-center text-text-primary text-[12px]">
      {@html message}
    </div>

    <!-- 底部按钮 -->
    <div class="flex items-center justify-center gap-4 px-6 pt-2 pb-4">
      <RoundButton
        customClass="w-22"
        label={cancelText}
        size="h-8"
        fontSize="text-sm"
        bgColor="bg-gray-200"
        textColor="text-gray-600"
        hoverColor="hover:bg-gray-300"
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
