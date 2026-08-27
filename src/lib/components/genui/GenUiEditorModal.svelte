<script lang="ts">
  /**
   * GenUI authoring, as a modal.
   *
   * A GenUI is edited from wherever it is used — the settings list, a tool's
   * detail while binding one — so it is a dialog rather than a route: leaving
   * the page you were configuring to go author a template, then finding your
   * way back, was the whole friction.
   *
   * Deleting deliberately lives with the LIST, not here: a confirm dialog
   * stacked on this one is two dialogs deep, and the list is where you already
   * see what you are about to remove.
   */
  import { Renderer, JsonUIProvider } from "@json-render/svelte";
  import type { Spec } from "@json-render/core";
  import Modal from "$lib/components/ui/Modal.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Input from "$lib/components/ui/Input.svelte";
  import { uiRegistry } from "./jsonui/registry";
  import { uiCatalog } from "./jsonui/catalog";
  import { explainSpec, type SpecDiagnosticStage } from "./jsonui/resolveSpec";
  import { genuiActions } from "$lib/states/genui.svelte";
  import { genuiExamples } from "./examples";
  import { t } from "$lib/i18n";
  import type { GenUi } from "$lib/types";

  interface Props {
    open: boolean;
    /** Existing template to edit; null opens the editor on a new one. */
    genui?: GenUi | null;
    onClose?: () => void;
    /** The saved record — lets a caller link it the moment it exists. */
    onSaved?: (genui: GenUi) => void;
  }

  let {
    open = $bindable(false),
    genui = null,
    onClose = () => {},
    onSaved = (_genui: GenUi) => {},
  }: Props = $props();

  /** Starting point for a new template, built from the shared catalog. */
  const seedSpec: Spec = {
    root: "card",
    elements: {
      card: {
        type: "Card",
        props: { title: "New GenUI" },
        children: ["stack"],
        visible: true,
      },
      stack: {
        type: "Stack",
        props: { gap: "md" },
        children: ["intro", "status"],
        visible: true,
      },
      intro: {
        type: "Text",
        props: {
          text: "Edit the JSON on the left; the right renders it live.",
          variant: "body",
        },
        children: [],
        visible: true,
      },
      status: {
        type: "StatusLabel",
        props: { status: "enabled", text: "Valid" },
        children: [],
        visible: true,
      },
    },
  };

  let name = $state("");
  let specInput = $state("");
  let saving = $state(false);
  let saveError = $state<string | null>(null);
  let loadedExampleId = $state<string | null>(null);

  const isEdit = $derived(Boolean(genui?.id));

  // Re-seed on every open: the same modal instance serves "edit this one",
  // "edit that one" and "create", so carrying the previous record's text over
  // would silently offer to overwrite the wrong template.
  let seededFor = $state<string | null>(null);
  $effect(() => {
    if (!open) {
      seededFor = null;
      return;
    }
    const key = genui?.id ?? "new";
    if (seededFor === key) return;
    name = genui?.name ?? "";
    specInput = genui?.spec ?? JSON.stringify(seedSpec, null, 2);
    saveError = null;
    loadedExampleId = null;
    seededFor = key;
  });

  // Example gallery (create mode): clicking loads the spec into the editor and
  // fills an empty name. Each example is pre-normalized through the same
  // pipeline as the live preview, so an invalid example just loses its
  // thumbnail instead of breaking the dialog.
  const examplePreviews = genuiExamples.map((example) => {
    const resolved = explainSpec(JSON.stringify(example.spec));
    return { ...example, preview: resolved.ok ? resolved.spec : null };
  });

  function loadExample(id: string) {
    const example = genuiExamples.find((candidate) => candidate.id === id);
    if (!example) return;
    specInput = JSON.stringify(example.spec, null, 2);
    if (!name.trim()) name = example.name;
    loadedExampleId = example.id;
  }

  // Bindings are allowed: this is the authoring surface for a custom tool's
  // view, where `{ $state: "/city" }` is the point. The strict pass still
  // guards a model's reply in the timeline. The preview has no state to read,
  // so a bound prop shows blank here; the tool detail previews it with sample
  // arguments.
  const result = $derived(explainSpec(specInput, { allowBindings: true }));
  const spec = $derived(result.ok ? result.spec : null);
  const error = $derived(result.ok ? null : result);
  const canSave = $derived(name.trim().length > 0 && result.ok && !saving);

  const stageLabels = $derived<Record<SpecDiagnosticStage, string>>({
    empty: t("settings.genui.editor.stage.empty"),
    json: t("settings.genui.editor.stage.json"),
    shape: t("settings.genui.editor.stage.shape"),
    components: t("settings.genui.editor.stage.components"),
    props: t("settings.genui.editor.stage.props"),
    references: t("settings.genui.editor.stage.references"),
  });

  const catalogComponents = Object.entries(uiCatalog.data.components).map(
    ([componentName, def]) => ({
      name: componentName,
      description: (def as { description?: string }).description ?? "",
    }),
  );

  async function handleSave() {
    if (!canSave) return;
    saving = true;
    saveError = null;
    try {
      const saved = genui?.id
        ? await genuiActions.updateGenui(genui.id, name.trim(), specInput)
        : await genuiActions.createGenui(name.trim(), specInput);
      onSaved(saved);
      open = false;
    } catch (e) {
      console.error("Failed to save GenUI:", e);
      saveError = t("settings.genui.editor.saveFailed");
    } finally {
      saving = false;
    }
  }
</script>

<Modal
  bind:open
  title={isEdit
    ? t("settings.genui.editor.editTitle")
    : t("settings.genui.editor.createTitle")}
  {onClose}
>
  <div class="flex max-h-[85vh] w-[min(1060px,88vw)] flex-col">
    <div
      class="flex items-end gap-3 border-b border-[var(--hairline)] px-6 pt-14 pb-4"
    >
      <div class="min-w-0 flex-1">
        <Input
          label={t("settings.genui.editor.name")}
          placeholder={t("settings.genui.editor.namePlaceholder")}
          bind:value={name}
          required
        />
      </div>
      <Button variant="primary" disabled={!canSave} onclick={handleSave}>
        {saving ? t("settings.genui.editor.saving") : t("common.save")}
      </Button>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-6 py-4">
      {#if saveError}
        <p class="mb-3 text-sm text-error">{saveError}</p>
      {/if}

      <div class="grid min-h-0 gap-4 lg:grid-cols-2">
        <div class="flex min-h-0 flex-col gap-1">
          <div class="text-xs text-base-content/60">
            {t("settings.genui.editor.spec")}
          </div>
          <textarea
            bind:value={specInput}
            spellcheck="false"
            class="min-h-80 w-full flex-1 resize-none rounded-lg border border-base-300 bg-base-100 p-3 font-mono text-xs leading-relaxed text-base-content focus:border-[var(--field-border-hover)] focus:outline-none"
          ></textarea>
        </div>

        <div class="flex min-h-0 flex-col gap-1">
          <div class="text-xs text-base-content/60">
            {t("settings.genui.editor.preview")}
          </div>
          <div
            class="min-h-80 flex-1 overflow-auto rounded-lg border border-base-300 bg-base-100 p-3"
          >
            {#if spec}
              <JsonUIProvider initialState={{}}>
                <Renderer {spec} registry={uiRegistry} />
              </JsonUIProvider>
            {:else if error}
              <div class="space-y-2">
                <div
                  class="inline-flex items-center gap-2 text-xs font-medium text-error"
                >
                  <span class="rounded bg-error/10 px-2 py-0.5"
                    >{stageLabels[error.stage]}</span
                  >
                  {t("settings.genui.editor.invalid")}
                </div>
                <pre
                  class="text-xs whitespace-pre-wrap break-words text-base-content/70">{error.message}</pre>
              </div>
            {/if}
          </div>
        </div>
      </div>

      {#if !isEdit}
        <section class="mt-6">
          <div class="mb-2 text-xs text-base-content/60">
            {t("settings.genui.editor.examples", {
              count: examplePreviews.length,
            })}
          </div>
          <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {#each examplePreviews as example (example.id)}
              <button
                type="button"
                onclick={() => loadExample(example.id)}
                title={example.description}
                class="group flex flex-col overflow-hidden rounded-lg border bg-base-100 text-left transition hover:shadow-sm {loadedExampleId ===
                example.id
                  ? 'border-primary ring-1 ring-primary'
                  : 'border-base-300 hover:border-primary/60'}"
              >
                <div class="border-b border-base-300 px-3 py-2">
                  <div class="flex items-center justify-between gap-2">
                    <span
                      class="truncate text-sm font-medium text-base-content"
                      >{example.name}</span
                    >
                    {#if loadedExampleId === example.id}
                      <span class="shrink-0 text-[10px] font-medium text-primary"
                        >{t("settings.genui.editor.exampleLoaded")}</span
                      >
                    {/if}
                  </div>
                  <div class="mt-0.5 line-clamp-2 text-xs text-base-content/55">
                    {example.description}
                  </div>
                </div>
                <div class="relative h-40 overflow-hidden bg-base-200/30 p-3">
                  {#if example.preview}
                    <div class="pointer-events-none">
                      <JsonUIProvider initialState={{}}>
                        <Renderer spec={example.preview} registry={uiRegistry} />
                      </JsonUIProvider>
                    </div>
                    <div
                      class="pointer-events-none absolute inset-x-0 bottom-0 h-10 bg-gradient-to-t from-base-100 to-transparent"
                    ></div>
                  {:else}
                    <div class="text-xs text-base-content/40">
                      {t("settings.genui.editor.previewUnavailable")}
                    </div>
                  {/if}
                </div>
              </button>
            {/each}
          </div>
        </section>
      {/if}

      <details class="mt-4 text-xs text-base-content/60">
        <summary class="cursor-pointer select-none">
          {t("settings.genui.editor.components", {
            count: catalogComponents.length,
          })}
        </summary>
        <ul class="mt-2 space-y-1">
          {#each catalogComponents as component (component.name)}
            <li>
              <span class="font-mono text-base-content/80">{component.name}</span
              > — {component.description}
            </li>
          {/each}
        </ul>
      </details>
    </div>
  </div>
</Modal>
