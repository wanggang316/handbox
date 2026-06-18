<script lang="ts">
  import Modal from "$lib/components/ui/Modal.svelte";
  import RoundButton from "$lib/components/ui/RoundButton.svelte";
  import { updateState } from "$lib/states/update.svelte";
  import { t } from "$lib/i18n";
  import { Download } from "@lucide/svelte";

  let modalRef = $state<Modal>();

  const downloading = $derived(updateState.status === "downloading");
  const hasTotal = $derived(updateState.contentLength > 0);
  const percent = $derived(Math.round(updateState.progress * 100));

  function handleUpdateNow() {
    updateState.startUpdate();
  }

  function handleLater() {
    modalRef?.handleClose();
  }

  // Modal 关闭动画结束后回调：保留侧边栏入口，仅关闭弹框
  function handleClosed() {
    updateState.remindLater();
  }
</script>

<Modal
  bind:this={modalRef}
  open={updateState.dialogOpen}
  showCloseButton={false}
  onClose={handleClosed}
>
  <div class="w-[380px] max-w-[90vw] flex flex-col px-6 pt-6 pb-5">
    <!-- 头部：版本对比 -->
    <div class="mb-3 flex items-center gap-3">
      <div
        class="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10 text-primary"
      >
        <Download size={20} />
      </div>
      <div class="flex flex-col">
        <h2 class="text-sm font-medium text-base-content">{t("update.newVersionFound")}</h2>
        <span class="text-[12px] text-base-content/60">
          v{updateState.currentVersion} → v{updateState.info?.version ?? ""}
        </span>
      </div>
    </div>

    <!-- 更新说明 -->
    {#if updateState.info?.body}
      <div
        class="mb-4 max-h-48 overflow-auto whitespace-pre-wrap rounded-lg bg-base-300/50 p-3 text-[12px] leading-relaxed text-base-content/80"
      >
        {updateState.info.body}
      </div>
    {/if}

    <!-- 下载进度 -->
    {#if downloading}
      <div class="mb-4">
        <div
          class="mb-1.5 flex items-center justify-between text-[12px] text-base-content/70"
        >
          <span>{t("update.downloading")}</span>
          {#if hasTotal}<span>{percent}%</span>{/if}
        </div>
        <div class="h-1.5 w-full overflow-hidden rounded-full bg-base-300">
          {#if hasTotal}
            <div
              class="h-full rounded-full bg-primary transition-[width] duration-150"
              style={`width:${percent}%`}
            ></div>
          {:else}
            <div class="h-full w-1/3 animate-pulse rounded-full bg-primary"></div>
          {/if}
        </div>
      </div>
    {/if}

    <!-- 操作按钮 -->
    <div class="flex items-center justify-end gap-3">
      <RoundButton
        label={t("update.remindLater")}
        size="h-8"
        fontSize="text-sm"
        variant="secondary"
        customClass="px-5"
        disabled={downloading}
        onclick={handleLater}
      />
      <RoundButton
        label={downloading ? t("update.updating") : t("update.updateNow")}
        size="h-8"
        fontSize="text-sm"
        variant="primary"
        customClass="px-5"
        disabled={downloading}
        onclick={handleUpdateNow}
      />
    </div>
  </div>
</Modal>
