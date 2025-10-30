<script lang="ts">
  import Modal from "./Modal.svelte";
  import RoundButton from "./RoundButton.svelte";

  // 定义操作按钮类型
  type ActionButton = {
    label: string;
    style?: "danger" | "accent" | "primary" | "secondary";
    onClick: () => void;
  };

  const {
    open = false,
    title = "确认操作",
    message = "确认要执行此操作吗？",
    confirmText = "确认",
    cancelText = "取消",
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
  const getButtonColors = (style?: string) => {
    switch (style) {
      case "danger":
        return {
          bgColor: "bg-error",
          textColor: "text-base-100",
          hoverColor: "hover:bg-error/90",
        };
      case "accent":
        return {
          bgColor: "bg-accent",
          textColor: "text-accent-content",
          hoverColor: "hover:bg-accent/90",
        };
      case "primary":
        return {
          bgColor: "bg-primary",
          textColor: "text-primary-content",
          hoverColor: "hover:bg-primary/90",
        };
      case "secondary":
        return {
          bgColor: "bg-base-200",
          textColor: "text-base-content/80",
          hoverColor: "hover:bg-base-300",
        };
      default:
        return {
          bgColor: "bg-primary",
          textColor: "text-primary-content",
          hoverColor: "hover:bg-primary/90",
        };
    }
  };

  const confirmColors = getButtonColors(confirmButtonStyle);

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
      <h2 class="text-sm">{title}</h2>
    </div>

    <!-- 内容 -->
    <div class="px-6 py-2 text-center text-base-content text-[12px]">
      {@html message}
    </div>

    <!-- 底部按钮 -->
    {#if actions && actions.length > 0}
      <!-- 多个操作按钮：垂直排列 -->
      <div class="flex flex-col gap-2 px-6 pt-2 pb-4">
        {#each actions as action}
          {@const colors = getButtonColors(action.style)}
          <RoundButton
            customClass="w-full"
            label={action.label}
            size="h-9"
            fontSize="text-sm"
            bgColor={colors.bgColor}
            textColor={colors.textColor}
            hoverColor={colors.hoverColor}
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
          label={isLoading ? "处理中..." : confirmText}
          bgColor={confirmColors.bgColor}
          textColor={confirmColors.textColor}
          hoverColor={confirmColors.hoverColor}
          disabled={isLoading}
          onclick={handleConfirm}
        />
      </div>
    {/if}
  </div>
</Modal>
