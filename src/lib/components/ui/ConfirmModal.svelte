<script lang="ts">
  import Modal from "./Modal.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import { t } from "$lib/i18n";

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
    confirmButtonStyle = "danger",
    autoCloseOnConfirm = true,
    actions = undefined, // When provided, replaces the default confirm/cancel buttons.
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
    actions?: ActionButton[];
    onClose?: () => void;
    onConfirm?: () => void;
    onCancel?: () => void;
  }>();

  // Fall back to localized defaults; $derived keeps them reactive to language switches.
  const resolvedTitle = $derived(title ?? t("ui.confirmTitle"));
  const resolvedMessage = $derived(message ?? t("ui.confirmMessage"));
  const resolvedConfirmText = $derived(confirmText ?? t("common.confirm"));
  const resolvedCancelText = $derived(cancelText ?? t("common.cancel"));

  let modalRef: Modal;

  export { modalRef };

  function handleConfirm() {
    onConfirm();
    if (autoCloseOnConfirm) {
      modalRef?.handleClose();
    }
  }

  function handleCancel() {
    onCancel();
    modalRef?.handleClose();
  }

  // Invoked by Modal after its close animation completes.
  function handleModalClose() {
    onClose();
  }

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
    <div class="flex items-center justify-center px-6 pt-4 pb-0">
      <h2 class="text-sm">{resolvedTitle}</h2>
    </div>

    <div class="px-6 py-2 text-center text-base-content text-[12px]">
      {@html resolvedMessage}
    </div>

    {#if actions && actions.length > 0}
      <div class="flex flex-col gap-2 px-6 pt-2 pb-4">
        {#each actions as action}
          <Button
            class="w-full"
            size="md"
            variant={action.style ?? "primary"}
            disabled={isLoading}
            onclick={() => handleActionClick(action)}
          >{action.label}</Button>
        {/each}
      </div>
    {:else}
      <div class="flex items-center justify-center gap-4 px-6 pt-2 pb-4">
        <Button
          class="w-22"
          size="md"
          variant="secondary"
          onclick={handleCancel}
        >{resolvedCancelText}</Button>
        <Button
          class="w-22"
          size="md"
          variant={confirmButtonStyle}
          disabled={isLoading}
          onclick={handleConfirm}
        >{isLoading ? t("ui.processing") : resolvedConfirmText}</Button>
      </div>
    {/if}
  </div>
</Modal>
