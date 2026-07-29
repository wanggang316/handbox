<script lang="ts">
  import Modal from "$lib/components/ui/Modal.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import { updateState } from "$lib/states/update.svelte";
  import { renderMarkdown, markdownInteractions } from "$lib/utils";
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

  // Runs after the close animation: dismiss only the dialog, keeping the sidebar entry.
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

    <!-- Release notes: latest.json's `notes`, authored as Keep a Changelog Markdown. -->
    {#if updateState.info?.body}
      <div
        class="update-notes markdown-content mb-4 max-h-48 overflow-auto rounded-lg bg-base-300/50 p-3 text-[12px] text-base-content/80 select-text"
        use:markdownInteractions
      >
        {@html renderMarkdown(updateState.info.body)}
      </div>
    {/if}

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
              class="h-full rounded-full bg-primary transition-[width] duration-[var(--dur-base)]"
              style={`width:${percent}%`}
            ></div>
          {:else}
            <div class="h-full w-1/3 animate-pulse rounded-full bg-primary"></div>
          {/if}
        </div>
      </div>
    {/if}

    <div class="flex items-center justify-end gap-3">
      <Button
        size="md"
        variant="secondary"
        class="px-5"
        disabled={downloading}
        onclick={handleLater}
      >{t("update.remindLater")}</Button>
      <Button
        size="md"
        variant="primary"
        class="px-5"
        disabled={downloading}
        onclick={handleUpdateNow}
      >{downloading ? t("update.updating") : t("update.updateNow")}</Button>
    </div>
  </div>
</Modal>

<style>
  /* Compact the global markdown typography for this narrow dialog, and restore
     the list markers Tailwind preflight strips. */
  .update-notes > :global(:first-child) {
    margin-top: 0;
  }

  .update-notes :global(h1),
  .update-notes :global(h2),
  .update-notes :global(h3),
  .update-notes :global(h4),
  .update-notes :global(h5),
  .update-notes :global(h6) {
    margin: 0.75rem 0 0.25rem;
    border: none;
    padding: 0;
    font-size: 12px;
    font-weight: 600;
    color: var(--base-content);
  }

  .update-notes :global(p),
  .update-notes :global(ul),
  .update-notes :global(ol) {
    margin: 0.25rem 0;
    line-height: 1.6;
  }

  .update-notes :global(ul),
  .update-notes :global(ol) {
    padding-left: 1.1rem;
  }

  .update-notes :global(ul) {
    list-style: disc;
  }

  .update-notes :global(ol) {
    list-style: decimal;
  }

  .update-notes :global(li) {
    margin: 0.125rem 0;
  }
</style>
