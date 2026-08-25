<script module lang="ts">
  /**
   * The installed-app probe is process-wide and stable for a session, so the
   * promise is cached at module scope: reopening the panel must not re-scan
   * /Applications and must not blank the editor row while it does.
   */
  let targetsPromise: Promise<OpenInTarget[]> | null = null;

  function loadOpenInTargetsOnce(): Promise<OpenInTarget[]> {
    targetsPromise ??= listOpenInTargets().catch((error) => {
      // Do not poison the cache: a failed probe should be retried next open.
      targetsPromise = null;
      throw error;
    });
    return targetsPromise;
  }
</script>

<script lang="ts">
  /**
   * Per-project settings, opened from the sidebar's project menu. Every field
   * writes through on change (no Save button): the panel is a settings sheet,
   * not a form, and the sidebar reflects a change the moment it lands.
   */
  import { untrack } from "svelte";
  import { Ban } from "@lucide/svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import { TableGroup, TableBaseRow, SelectRow } from "$lib/components/ui/table";
  import { listOpenInTargets, type OpenInTarget } from "$lib/api/openIn";
  import { agentProjectActions } from "$lib/states/agentProject.svelte";
  import { settingsState } from "$lib/states/settings.svelte";
  import { t } from "$lib/i18n";
  import { normalizeError } from "$lib/utils/error";
  import type { AgentProject } from "$lib/types/agentProject";

  interface Props {
    open?: boolean;
    /** The project being edited; null while the panel has never been opened. */
    project?: AgentProject | null;
  }

  let { open = $bindable(false), project = null }: Props = $props();

  /** Swatch palette (macOS system colors); the value stored is the hex itself. */
  const PRESET_COLORS = [
    "#ff3b30",
    "#ff9500",
    "#ffcc00",
    "#34c759",
    "#007aff",
    "#af52de",
    "#8e8e93",
  ];

  /** Editor row value standing for "no override"; stored as null. */
  const GLOBAL_EDITOR_VALUE = "";

  // Draft state. Mirrors the project until a write lands, so the name input can
  // hold an uncommitted edit without the row jumping under the cursor.
  let name = $state("");
  let color = $state<string | null>(null);
  /** Bound to the Select, so "no override" travels as GLOBAL_EDITOR_VALUE. */
  let editorValue = $state(GLOBAL_EDITOR_VALUE);
  let error = $state<string | null>(null);

  let targets = $state<OpenInTarget[]>([]);

  // Which project the draft above was seeded from; re-seeding is keyed on it so
  // the store's own update (after a successful save) does not reset a field
  // being edited.
  let seededId = "";

  $effect(() => {
    const current = project;
    if (!open || !current) {
      seededId = "";
      return;
    }
    if (current.id === seededId) return;
    seededId = current.id;
    untrack(() => {
      name = current.name;
      color = current.color ?? null;
      editorValue = current.defaultEditorId ?? GLOBAL_EDITOR_VALUE;
      error = null;
    });
  });

  // The project is read live from the store, so deleting it from the sidebar
  // while the panel is up would leave an empty dialog behind.
  $effect(() => {
    if (open && !project) open = false;
  });

  // Probe the installed editors while the panel opens, and make sure settings
  // are loaded so the "Global — ..." label can name the real fallback.
  $effect(() => {
    if (!open) return;
    void settingsState.loadSettings();
    loadOpenInTargetsOnce()
      .then((list) => (targets = list))
      .catch((probeError) =>
        console.error("Failed to list open-in targets:", probeError),
      );
  });

  // Finder is not an "editor": the global default excludes it (see
  // AgentSessionHeader), and so does this list.
  const editorTargets = $derived(
    targets.filter((target) => target.kind !== "system"),
  );

  const globalEditorName = $derived.by(() => {
    const globalId = settingsState.settings?.agent?.defaultEditorId ?? null;
    return (
      editorTargets.find((target) => target.id === globalId)?.name ??
      editorTargets[0]?.name ??
      null
    );
  });

  const editorOptions = $derived([
    {
      value: GLOBAL_EDITOR_VALUE,
      label: globalEditorName
        ? t("agent.projectSettings.globalEditor", { name: globalEditorName })
        : t("agent.projectSettings.globalEditorUnset"),
    },
    ...editorTargets.map((target) => ({
      value: target.id,
      label: target.name,
    })),
  ]);

  /**
   * Write the three fields as one group. Nothing is sent when the draft matches
   * what is stored, so a blur that changed nothing is not a write.
   */
  async function save(next: {
    name: string;
    color: string | null;
    defaultEditorId: string | null;
  }): Promise<void> {
    const current = project;
    if (!current) return;
    if (
      next.name === current.name &&
      next.color === (current.color ?? null) &&
      next.defaultEditorId === (current.defaultEditorId ?? null)
    ) {
      return;
    }

    error = null;
    try {
      await agentProjectActions.updateProjectSettings(current.id, next);
    } catch (saveError) {
      // Revert the draft to what is actually stored: leaving a rejected value
      // on screen would claim a save that never happened.
      name = current.name;
      color = current.color ?? null;
      editorValue = current.defaultEditorId ?? GLOBAL_EDITOR_VALUE;
      const normalized = normalizeError(
        saveError,
        t("agent.projectSettings.saveFailed"),
      );
      error = normalized.hint ?? normalized.message;
    }
  }

  /** The current draft as a settings group, with the edited field overridden. */
  function draft(
    overrides: Partial<{
      name: string;
      color: string | null;
      defaultEditorId: string | null;
    }>,
  ) {
    return {
      name: name.trim(),
      color,
      defaultEditorId: editorValue || null,
      ...overrides,
    };
  }

  // Name commits on blur / Enter. A blank name is not a write — the input snaps
  // back, matching the backend rule that a project must stay named.
  function commitName() {
    const trimmed = name.trim();
    if (!trimmed) {
      name = project?.name ?? "";
      return;
    }
    name = trimmed;
    void save(draft({ name: trimmed }));
  }

  function handleNameKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      (event.currentTarget as HTMLInputElement).blur();
    } else if (event.key === "Escape") {
      name = project?.name ?? "";
      (event.currentTarget as HTMLInputElement).blur();
    }
  }

  function pickColor(next: string | null) {
    color = next;
    void save(draft({ color: next }));
  }

  // `editorValue` is already updated by the binding; only the write is left.
  function pickEditor(value: string) {
    void save(draft({ defaultEditorId: value || null }));
  }

  // The custom picker opens on whatever is currently set, falling back to a
  // neutral gray rather than the browser default (black).
  const customColorValue = $derived(color ?? "#8e8e93");
  const isPreset = $derived(color !== null && PRESET_COLORS.includes(color));
</script>

<Modal bind:open>
  <div class="flex w-[560px] max-w-[92vw] max-h-[86vh] flex-col">
    {#if project}
      <!-- pt-14 clears Modal's traffic-light row. -->
      <div class="flex-shrink-0 px-7 pt-14">
        <h2 class="truncate text-[17px] font-semibold text-base-content">
          {t("agent.projectSettings.title", { name: project.name })}
        </h2>
        <p class="mt-1 truncate text-[13px] text-base-content/50">
          {project.path}
        </p>
        {#if error}
          <p class="mt-3 text-[13px] text-error">{error}</p>
        {/if}
      </div>

      <div class="flex flex-1 min-h-0 flex-col gap-y-4 overflow-y-auto px-7 pb-7 pt-5">
        <TableGroup title={t("agent.projectSettings.general")}>
          <!-- Not TextRow: the name is committed on blur / Enter rather than on
               every keystroke, which needs the input's own handlers. -->
          <TableBaseRow label={t("agent.projectSettings.name")} py="2">
            <input
              class="w-full border-none p-1 text-right text-sm text-base-content"
              bind:value={name}
              onblur={commitName}
              onkeydown={handleNameKeydown}
              placeholder={t("agent.list.renamePlaceholder")}
            />
          </TableBaseRow>

          <TableBaseRow label={t("agent.projectSettings.color")}>
            <div class="flex items-center gap-2">
              <button
                type="button"
                class="flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-full text-base-content/60 hover:text-base-content {color ===
                null
                  ? 'outline outline-2 outline-offset-2 outline-base-content/40'
                  : ''}"
                title={t("agent.projectSettings.colorNone")}
                aria-label={t("agent.projectSettings.colorNone")}
                aria-pressed={color === null}
                onclick={() => pickColor(null)}
              >
                <Ban size={22} />
              </button>

              {#each PRESET_COLORS as preset (preset)}
                <button
                  type="button"
                  class="h-6 w-6 flex-shrink-0 rounded-full {color === preset
                    ? 'outline outline-2 outline-offset-2 outline-base-content/40'
                    : ''}"
                  style="background-color: {preset};"
                  aria-label={preset}
                  aria-pressed={color === preset}
                  onclick={() => pickColor(preset)}
                ></button>
              {/each}

              <!-- Custom color: a native picker behind a color-wheel swatch. -->
              <span
                class="relative h-6 w-6 flex-shrink-0 rounded-full {color !==
                  null && !isPreset
                  ? 'outline outline-2 outline-offset-2 outline-base-content/40'
                  : ''}"
                style="background: conic-gradient(#ff3b30, #ffcc00, #34c759, #00c7be, #007aff, #af52de, #ff3b30);"
                title={t("agent.projectSettings.colorCustom")}
              >
                <input
                  type="color"
                  class="absolute inset-0 h-full w-full cursor-pointer opacity-0"
                  aria-label={t("agent.projectSettings.colorCustom")}
                  value={customColorValue}
                  oninput={(event) =>
                    pickColor((event.currentTarget as HTMLInputElement).value)}
                />
              </span>
            </div>
          </TableBaseRow>
        </TableGroup>

        <TableGroup title={t("agent.projectSettings.editor")}>
          <SelectRow
            label={t("agent.projectSettings.defaultEditor")}
            options={editorOptions}
            bind:selectedValue={editorValue}
            onSelect={pickEditor}
          />
        </TableGroup>
      </div>
    {/if}
  </div>
</Modal>
