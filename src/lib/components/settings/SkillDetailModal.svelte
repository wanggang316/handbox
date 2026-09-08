<script lang="ts">
  /**
   * A skill, shown as the directory it actually is.
   *
   * The list used to inline `SKILL.md` behind a "view content" disclosure,
   * which could only ever show one file and pushed every row below it off
   * screen. A skill is prose plus references plus scripts, so the detail is a
   * dialog whose whole directory collapses into one picker in the toolbar.
   *
   * The dialog is a fixed size and every file read is cached, so switching
   * files changes the text and nothing else — no resize, no spinner in the
   * place the content was.
   */
  import { FolderOpen, FileText } from "@lucide/svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import Spinner from "$lib/components/ui/Spinner.svelte";
  import CodePreview from "$lib/components/ui/CodePreview.svelte";
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
  let selected = $state("");
  let content = $state("");
  let reading = $state(false);
  let error = $state<string | null>(null);
  let scroller = $state<HTMLDivElement | null>(null);

  /**
   * Every file read so far, keyed by relative path. A file already visited
   * switches back synchronously, which is the difference between switching and
   * flashing.
   */
  let cache = new Map<string, string>();

  const selectedFile = $derived(
    files.find((f) => f.relPath === selected) ?? null,
  );
  const isMarkdown = $derived(selected.toLowerCase().endsWith(".md"));
  const fileOptions = $derived(
    files.map((f) => ({ value: f.relPath, label: f.relPath })),
  );

  async function select(relPath: string): Promise<void> {
    if (!relPath || relPath === selected) return;
    selected = relPath;
    error = null;

    const cached = cache.get(relPath);
    if (cached !== undefined) {
      show(cached);
      return;
    }
    if (!skill || !files.find((f) => f.relPath === relPath)?.readable) {
      show("");
      return;
    }

    // The previous file stays on screen until the next one arrives: blanking
    // the pane for a local read that takes a millisecond is the flash itself.
    reading = true;
    const name = skill.name;
    try {
      const text = await readSkillFile(name, relPath);
      cache.set(relPath, text);
      if (selected === relPath) show(text);
    } catch (e) {
      console.error("Failed to read skill file:", e);
      if (selected === relPath) {
        content = "";
        error = t("settings.skills.detail.loadFailed");
      }
    } finally {
      if (selected === relPath) reading = false;
    }
  }

  /** Swap the text and return to the top: a new file starts at its first line. */
  function show(text: string): void {
    content = text;
    if (scroller) scroller.scrollTop = 0;
  }

  // Reload whenever the dialog is opened on a skill. A listing failure is not
  // fatal: SKILL.md is already in hand, so the dialog still shows the skill.
  $effect(() => {
    if (!open || !skill) return;

    const name = skill.name;
    const body = skill.body;
    cache = new Map(body === null ? [] : [["SKILL.md", body]]);
    files = body === null ? [] : [{ relPath: "SKILL.md", size: 0, readable: true }];
    selected = "SKILL.md";
    content = body ?? "";
    error = null;

    listSkillFiles(name)
      .then((listed) => {
        if (skill?.name !== name) return;
        files = listed;
        if (!listed.some((f) => f.relPath === selected)) {
          void select(listed[0]?.relPath ?? "");
        }
      })
      .catch((e) => console.error("Failed to list skill files:", e));
  });
</script>

<Modal bind:open {onClose}>
  <!-- Fixed size: the dialog is a reading pane, and one that resized itself
       around each file made every switch feel like a new window. The width is
       what prose and a wrapped code line want, not what a file column did. -->
  <div class="flex h-[78vh] w-[min(860px,90vw)] flex-col">
    <div class="flex items-start gap-4 px-6 pt-14 pb-5">
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
          <p class="mt-1 line-clamp-3 text-sm leading-relaxed text-base-content/60">
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

    <!-- The whole directory lives in this one control. A skill with a single
         file has nothing to switch between, so it shows the name instead. -->
    <div
      class="flex items-center gap-3 border-y border-[var(--hairline)] px-6 py-2.5"
    >
      {#if files.length > 1}
        <Select
          autoWidth
          size="sm"
          align="start"
          options={fileOptions}
          value={selected}
          onChange={select}
        />
      {:else}
        <span class="flex items-center gap-1.5 font-mono text-xs text-base-content/60">
          <FileText size={14} />
          {selected}
        </span>
      {/if}

      {#if reading}
        <Spinner size={14} />
      {/if}

      {#if selectedFile && selectedFile.size > 0}
        <span class="ml-auto text-xs text-base-content/35">
          {formatFileSize(selectedFile.size)}
        </span>
      {/if}
    </div>

    <div
      bind:this={scroller}
      class="min-h-0 flex-1 overflow-auto px-6 py-5 select-text"
    >
      {#if error}
        <p class="text-sm text-error">{error}</p>
      {:else if selectedFile && !selectedFile.readable}
        <p class="text-sm text-base-content/50">
          {t("settings.skills.detail.notReadable", {
            size: formatFileSize(selectedFile.size),
          })}
        </p>
      {:else if isMarkdown}
        <div class="markdown-content text-[13px]" use:markdownInteractions>
          {@html renderMarkdown(content)}
        </div>
      {:else}
        <CodePreview code={content} filename={selected} />
      {/if}
    </div>
  </div>
</Modal>
