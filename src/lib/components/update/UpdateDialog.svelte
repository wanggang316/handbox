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
  <div class="w-[560px] max-w-[90vw] flex flex-col">
    <div
      class="flex items-center gap-3 px-6 pt-5 pb-4 border-b border-[var(--hairline)]"
    >
      <div
        class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary"
      >
        <Download size={20} />
      </div>
      <div class="flex flex-col gap-0.5">
        <h2 class="text-sm font-medium text-base-content">{t("update.newVersionFound")}</h2>
        <span class="text-[12px] text-base-content/60">
          v{updateState.currentVersion} → v{updateState.info?.version ?? ""}
        </span>
      </div>
    </div>

    <!-- Release notes: latest.json's `notes`, authored as Keep a Changelog Markdown. -->
    {#if updateState.info?.body}
      <div
        class="update-notes markdown-content max-h-[50vh] min-h-[96px] overflow-y-auto px-6 py-4 text-[13px] text-base-content/80 select-text"
        use:markdownInteractions
      >
        {@html renderMarkdown(updateState.info.body)}
      </div>
    {/if}

    {#if downloading}
      <div class="px-6 pt-1 pb-4">
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

    <div
      class="flex items-center justify-end gap-3 px-6 pt-3 pb-4 border-t border-[var(--hairline)]"
    >
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
  /* Compact the global markdown typography for this dialog, and restore the list
     markers Tailwind preflight strips. Changelog sections arrive as headings of
     whatever level the CHANGELOG used, so all six render as one small label. */
  .update-notes > :global(:first-child) {
    margin-top: 0;
  }

  .update-notes :global(h1),
  .update-notes :global(h2),
  .update-notes :global(h3),
  .update-notes :global(h4),
  .update-notes :global(h5),
  .update-notes :global(h6) {
    margin: 1.1rem 0 0.4rem;
    border: none;
    padding: 0;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: color-mix(in oklch, var(--base-content) 55%, transparent);
  }

  .update-notes :global(p),
  .update-notes :global(ul),
  .update-notes :global(ol) {
    margin: 0.35rem 0;
    line-height: 1.7;
  }

  .update-notes :global(ul),
  .update-notes :global(ol) {
    padding-left: 1.2rem;
  }

  .update-notes :global(ul) {
    list-style: disc;
  }

  .update-notes :global(ol) {
    list-style: decimal;
  }

  .update-notes :global(li) {
    margin: 0.2rem 0;
    line-height: 1.7;
  }
</style>
