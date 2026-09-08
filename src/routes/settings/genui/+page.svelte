<script lang="ts">
  /**
   * GenUI templates.
   *
   * Moved out of the Agents page's second tab: a GenUI is bound by an agent AND
   * by a custom tool, so filing it under "agents" undersold what it is. Editing
   * happens in a dialog rather than a route — see `GenUiEditorModal`.
   */
  import { onMount } from "svelte";
  import { LayoutTemplate, Plus, Trash2 } from "@lucide/svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Spinner from "$lib/components/ui/Spinner.svelte";
  import ConfirmModal from "$lib/components/ui/ConfirmModal.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import TableBaseRow from "$lib/components/ui/table/TableBaseRow.svelte";
  import GenUiEditorModal from "$lib/components/genui/GenUiEditorModal.svelte";
  import { genuiState, genuiActions } from "$lib/states/genui.svelte";
  import { t } from "$lib/i18n";
  import type { GenUi } from "$lib/types";

  let editorOpen = $state(false);
  /** The template the editor is opened on; null creates a new one. */
  let editing = $state<GenUi | null>(null);
  let deleteTarget = $state<GenUi | null>(null);
  let deleteError = $state<string | null>(null);

  onMount(() => {
    genuiActions.loadGenuis().catch((error) => {
      console.error("Failed to load GenUIs:", error);
    });
  });

  function openCreate(): void {
    editing = null;
    editorOpen = true;
  }

  function openEdit(genui: GenUi): void {
    editing = genui;
    editorOpen = true;
  }

  async function confirmDelete(): Promise<void> {
    const target = deleteTarget;
    if (!target?.id) return;
    try {
      await genuiActions.deleteGenui(target.id);
      deleteTarget = null;
    } catch (error) {
      console.error("Failed to delete GenUI:", error);
      deleteError = t("settings.genui.deleteFailed");
    }
  }
</script>

<div class="flex flex-col gap-y-4 p-6 pt-2 pr-8">
  <div class="flex items-start justify-between gap-4">
    <p class="text-sm text-base-content/60">
      {t("settings.genui.description")}
    </p>
    <Button variant="secondary" size="sm" class="shrink-0" onclick={openCreate}>
      <Plus size={14} />
      {t("settings.genui.new")}
    </Button>
  </div>

  {#if deleteError}
    <p class="text-sm text-error">{deleteError}</p>
  {/if}

  <!-- Spinner only on a cold list: with templates already cached, repainting
       them beats flashing a spinner over content that is about to be identical. -->
  {#if genuiState.isLoading && genuiState.genuis.length === 0}
    <div class="flex justify-center py-16"><Spinner /></div>
  {:else if genuiState.genuis.length === 0}
    <div
      class="flex flex-col items-center justify-center gap-2 py-16 text-base-content/50"
    >
      <LayoutTemplate size={40} class="opacity-20" />
      <p class="text-sm">{t("settings.genui.empty")}</p>
      <p class="text-[13px] text-base-content/40">
        {t("settings.genui.emptyHint")}
      </p>
    </div>
  {:else}
    <TableGroup title={t("settings.genui.count", { count: genuiState.genuis.length })}>
      {#each genuiState.genuis as genui (genui.id)}
        <TableBaseRow label={genui.name} icon={rowIcon}>
          <div class="flex items-center gap-1">
            <Button
              variant="secondary"
              size="sm"
              onclick={() => openEdit(genui)}
            >
              {t("common.edit")}
            </Button>
            <Button
              variant="clear"
              size="icon-sm"
              ariaLabel={t("settings.genui.deleteTitle")}
              class="text-base-content/40 enabled:hover:text-error"
              onclick={() => {
                deleteError = null;
                deleteTarget = genui;
              }}
            >
              <Trash2 size={16} />
            </Button>
          </div>
        </TableBaseRow>
      {/each}
    </TableGroup>
  {/if}
</div>

{#snippet rowIcon(props: { class: string })}
  <LayoutTemplate class={props.class} />
{/snippet}

<GenUiEditorModal bind:open={editorOpen} genui={editing} />

<ConfirmModal
  open={deleteTarget !== null}
  title={t("settings.genui.deleteTitle")}
  message={t("settings.genui.deleteMessage", { name: deleteTarget?.name ?? "" })}
  confirmText={t("common.delete")}
  confirmButtonStyle="danger"
  onConfirm={confirmDelete}
  onClose={() => (deleteTarget = null)}
/>
