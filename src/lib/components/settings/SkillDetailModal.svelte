<script lang="ts">
  /**
   * A skill, shown as the directory it actually is.
   *
   * The list used to inline `SKILL.md` behind a "view content" disclosure,
   * which could only ever show one file and pushed every row below it off
   * screen. A skill is prose plus references plus scripts, so the detail is a
   * dialog with a file switcher: the list stays a list, and the content gets
   * the width it needs.
   */
  import { Package, FolderOpen } from "@lucide/svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import Spinner from "$lib/components/ui/Spinner.svelte";
  import { listSkillFiles, readSkillFile } from "$lib/api/skill";
  import { renderMarkdown, markdownInteractions } from "$lib/utils";
  import { formatFileSize } from "$lib/utils/format";
  import { t } from "$lib/i18n";
  import type { SkillFile, SkillInfo } from "$lib/types";

  interface Props {
    open: boolean;
    /** The skill to show; null while the dialog is closed. */
    skill?: SkillInfo | null;
    /** Same non-optimistic commit the list row uses; omitted hides the toggle. */
    onToggleBefore?: (enabled: boolean) => Promise<boolean>;
    onOpenDir?: () => void;
    onClose?: () => void;
  }

  let {
    open = $bindable(false),
    skill = null,
    onToggleBefore,
    onOpenDir,
    onClose = () => {},
  }: Props = $props();

  let files = $state<SkillFile[]>([]);
  let selected = $state<string>("");
  let content = $state<string>("");
  let listing = $state(false);
  let reading = $state(false);
  let error = $state<string | null>(null);

  const selectedFile = $derived(files.find((f) => f.relPath === selected) ?? null);
  const isMarkdown = $derived(selected.toLowerCase().endsWith(".md"));

  /**
   * `SKILL.md` is already in hand from discovery, so it renders on the first
   * frame and only the other files cost a round trip.
   */
  function cachedContent(relPath: string): string | null {
    return relPath === "SKILL.md" ? (skill?.body ?? null) : null;
  }

  async function select(relPath: string): Promise<void> {
    selected = relPath;
    error = null;

    const cached = cachedContent(relPath);
    if (cached !== null) {
      content = cached;
      return;
    }
    if (!skill || !files.find((f) => f.relPath === relPath)?.readable) {
      content = "";
      return;
    }

    reading = true;
    try {
      content = await readSkillFile(skill.name, relPath);
    } catch (e) {
      console.error("Failed to read skill file:", e);
      content = "";
      error = t("settings.skills.detail.loadFailed");
    } finally {
      reading = false;
    }
  }

  // Reload whenever the dialog is opened on a skill. Listing failures are not
  // fatal: SKILL.md is already known, so the dialog still shows the skill.
  $effect(() => {
    if (!open || !skill) return;

    const name = skill.name;
    files = skill.body === null ? [] : [{ relPath: "SKILL.md", size: 0, readable: true }];
    selected = "SKILL.md";
    content = skill.body ?? "";
    error = null;
    listing = true;

    listSkillFiles(name)
      .then((listed) => {
        if (skill?.name !== name) return;
        files = listed;
        if (!listed.some((f) => f.relPath === selected)) {
          void select(listed[0]?.relPath ?? "");
        }
      })
      .catch((e) => console.error("Failed to list skill files:", e))
      .finally(() => {
        listing = false;
      });
  });
</script>

<Modal bind:open {onClose}>
  <div class="flex max-h-[85vh] w-[min(1180px,90vw)] flex-col">
    <div class="flex items-start gap-4 px-6 pt-14 pb-5">
      <div
        class="flex size-11 shrink-0 items-center justify-center rounded-full border border-[var(--hairline)] text-base-content/70"
      >
        <Package size={20} />
      </div>

      <div class="min-w-0 flex-1">
        <div class="flex items-baseline gap-2">
          <h2 class="truncate text-xl font-semibold text-base-content">
            {skill?.name ?? ""}
          </h2>
          <span class="shrink-0 text-lg text-base-content/40">
            {t("settings.skills.detail.badge")}
          </span>
        </div>
        {#if skill?.description}
          <p class="mt-1 text-sm leading-relaxed text-base-content/60">
            {skill.description}
          </p>
        {/if}
      </div>

      <div class="flex shrink-0 items-center gap-2 pt-1">
        {#if onToggleBefore && skill && skill.diagnostics.length === 0}
          <Toggle checked={!skill.disabled} onChangeBefore={onToggleBefore} />
        {/if}
        {#if onOpenDir}
          <button
            type="button"
            class="rounded-md p-1.5 text-base-content/45 transition-colors hover:bg-base-content/10 hover:text-base-content"
            title={t("settings.skills.openDir")}
            aria-label={t("settings.skills.openDir")}
            onclick={onOpenDir}
          >
            <FolderOpen size={16} />
          </button>
        {/if}
      </div>
    </div>

    {#if skill && skill.diagnostics.length > 0}
      <div class="mx-6 mb-4 rounded-lg bg-error/10 px-4 py-3 text-sm text-error">
        {#each skill.diagnostics as diagnostic}
          <p class="break-words">{diagnostic}</p>
        {/each}
      </div>
    {/if}

    <!-- The switcher earns its column only for a skill that has more than one
         file; a single-file skill gets the full width for its prose. -->
    <div class="flex min-h-0 flex-1 border-t border-[var(--hairline)]">
      {#if files.length > 1}
        <nav
          class="w-56 shrink-0 overflow-y-auto border-r border-[var(--hairline)] p-2"
        >
          {#each files as file (file.relPath)}
            <button
              type="button"
              class="mb-0.5 block w-full truncate rounded-md px-2.5 py-1.5 text-left font-mono text-xs transition-colors {file.relPath ===
              selected
                ? 'bg-base-200 text-base-content'
                : 'text-base-content/55 hover:bg-base-200/60 hover:text-base-content/85'} {file.readable
                ? ''
                : 'opacity-50'}"
              title={file.relPath}
              onclick={() => select(file.relPath)}
            >
              {file.relPath}
            </button>
          {/each}
        </nav>
      {/if}

      <div class="min-w-0 flex-1 overflow-y-auto px-6 py-5 select-text">
        {#if listing && files.length === 0}
          <div class="flex justify-center py-10"><Spinner size={24} /></div>
        {:else if error}
          <p class="text-sm text-error">{error}</p>
        {:else if selectedFile && !selectedFile.readable}
          <p class="text-sm text-base-content/50">
            {t("settings.skills.detail.notReadable", {
              size: formatFileSize(selectedFile.size),
            })}
          </p>
        {:else if reading}
          <div class="flex justify-center py-10"><Spinner size={24} /></div>
        {:else if isMarkdown}
          <div class="markdown-content text-[13px]" use:markdownInteractions>
            {@html renderMarkdown(content)}
          </div>
        {:else}
          <pre
            class="whitespace-pre-wrap break-words font-mono text-xs leading-relaxed text-base-content/80">{content}</pre>
        {/if}
      </div>
    </div>
  </div>
</Modal>
