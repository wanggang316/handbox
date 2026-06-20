<script lang="ts">
  import { goto } from "$app/navigation";
  import { ArrowLeft, Save, Trash2 } from "@lucide/svelte";
  import { Renderer, JsonUIProvider } from "@json-render/svelte";
  import type { Spec } from "@json-render/core";
  import { uiRegistry } from "$lib/components/chat/renderers/jsonui/registry";
  import { uiCatalog } from "$lib/components/chat/renderers/jsonui/catalog";
  import {
    explainSpec,
    type SpecDiagnosticStage,
  } from "$lib/components/chat/renderers/jsonui/resolveSpec";
  import Button from "$lib/components/ui/Button.svelte";
  import Input from "$lib/components/ui/Input.svelte";
  import ConfirmModal from "$lib/components/ui/ConfirmModal.svelte";
  import { genuiActions } from "$lib/states/genui.svelte";
  import { genuiExamples } from "./examples";
  import type { GenUi } from "$lib/types";

  interface Props {
    // 编辑模式传入既有 GenUI；新建模式留空
    genui?: GenUi | null;
  }

  let { genui = null }: Props = $props();

  // 新建时的起始 spec：与 chat 渲染同一套 catalog，编辑左侧 JSON 右侧实时预览。
  const seedSpec: Spec = {
    root: "card",
    elements: {
      card: { type: "Card", props: { title: "新建 GenUI" }, children: ["stack"], visible: true },
      stack: { type: "Stack", props: { gap: "md" }, children: ["intro", "status"], visible: true },
      intro: {
        type: "Text",
        props: { text: "编辑左侧 JSON，右侧实时渲染。保存后可在 Agent 表单中关联。", variant: "body" },
        children: [],
        visible: true,
      },
      status: {
        type: "StatusLabel",
        props: { status: "enabled", text: "实时校验通过" },
        children: [],
        visible: true,
      },
    },
  };

  let name = $state(genui?.name ?? "");
  let specInput = $state(genui?.spec ?? JSON.stringify(seedSpec, null, 2));
  let saving = $state(false);
  let showDeleteConfirm = $state(false);

  const isEdit = $derived(Boolean(genui?.id));

  // 示例库：把内置模板渲染成可点击的卡片（新建模式，置于编辑器下方）。点击即把 spec
  // 写入左侧文本域、空名称时顺带填名，并把该卡片高亮为「已载入」。每个示例预先过一遍
  // explainSpec 做归一化（与右侧实时预览同一管线），非法示例只是缩略图不渲染，而不会
  // 整页报错。explainSpec 改写的是 JSON.stringify 出来的副本，不会污染原始 example。
  let loadedExampleId = $state<string | null>(null);
  const examplePreviews = genuiExamples.map((ex) => {
    const resolved = explainSpec(JSON.stringify(ex.spec));
    return { ...ex, preview: resolved.ok ? resolved.spec : null };
  });

  function loadExample(id: string) {
    const ex = genuiExamples.find((e) => e.id === id);
    if (!ex) return;
    specInput = JSON.stringify(ex.spec, null, 2);
    if (!name.trim()) name = ex.name;
    loadedExampleId = ex.id;
  }

  // 走与聊天相同的 resolveSpec 校验管线；非法时报告失败阶段与原因。
  const result = $derived(explainSpec(specInput));
  const spec = $derived(result.ok ? result.spec : null);
  const error = $derived(result.ok ? null : result);
  const canSave = $derived(name.trim().length > 0 && result.ok && !saving);

  const stageLabels: Record<SpecDiagnosticStage, string> = {
    empty: "空输入",
    json: "JSON 语法",
    shape: "顶层结构",
    components: "组件 / 结构",
    props: "组件 props",
    references: "引用完整性",
  };

  const catalogComponents = Object.entries(uiCatalog.data.components).map(
    ([cname, def]) => ({
      name: cname,
      description: (def as { description?: string }).description ?? "",
    }),
  );

  function backToList() {
    goto("/agents?tab=genui");
  }

  async function handleSave() {
    if (!canSave) return;
    saving = true;
    try {
      if (genui?.id) {
        await genuiActions.updateGenui(genui.id, name.trim(), specInput);
      } else {
        await genuiActions.createGenui(name.trim(), specInput);
      }
      backToList();
    } catch (e) {
      console.error("Failed to save GenUI:", e);
      alert("保存 GenUI 失败");
    } finally {
      saving = false;
    }
  }

  async function handleDelete() {
    if (!genui?.id) return;
    try {
      await genuiActions.deleteGenui(genui.id);
      showDeleteConfirm = false;
      backToList();
    } catch (e) {
      console.error("Failed to delete GenUI:", e);
      alert("删除 GenUI 失败");
    }
  }
</script>

<div class="h-full flex flex-col">
  <!-- 顶部工具栏 -->
  <div class="flex-shrink-0 p-4 border-b border-base-300 mt-12">
    <button
      class="flex items-center gap-2 text-sm text-base-content/70 hover:text-base-content w-fit mb-4"
      onclick={backToList}
    >
      <ArrowLeft size={14} />
      返回列表
    </button>

    <div class="flex items-end justify-between gap-4">
      <div class="flex-1 max-w-md">
        <Input label="名称" placeholder="为这份 GenUI 取个名字" bind:value={name} required />
      </div>
      <div class="flex items-center gap-2">
        {#if isEdit}
          <Button
            variant="danger"
            size="sm"
            onclick={() => (showDeleteConfirm = true)}
            customClass="flex items-center gap-2"
          >
            <Trash2 size={14} />
            删除
          </Button>
        {/if}
        <Button
          variant="primary"
          size="sm"
          onclick={handleSave}
          disabled={!canSave}
          customClass="flex items-center gap-2"
        >
          <Save size={14} />
          {saving ? "保存中…" : "保存"}
        </Button>
      </div>
    </div>
  </div>

  <!-- 编辑器主体：左 JSON / 右实时渲染 -->
  <div class="flex-1 min-h-0 overflow-y-auto p-4">
    <div class="grid gap-4 lg:grid-cols-2 min-h-0 {isEdit ? 'h-full' : ''}">
      <div class="flex flex-col gap-1 min-h-0">
        <div class="text-xs text-base-content/60">spec JSON</div>
        <textarea
          bind:value={specInput}
          spellcheck="false"
          class="w-full flex-1 min-h-96 rounded-lg border border-base-300 bg-base-100 p-3 font-mono text-xs leading-relaxed text-base-content focus:outline-none focus:ring-1 focus:ring-primary resize-none"
        ></textarea>
      </div>

      <div class="flex flex-col gap-1 min-h-0">
        <div class="text-xs text-base-content/60">渲染结果</div>
        <div class="flex-1 min-h-96 rounded-lg border border-base-300 bg-base-100 p-3 overflow-auto">
          {#if spec}
            <JsonUIProvider initialState={{}}>
              <Renderer {spec} registry={uiRegistry} />
            </JsonUIProvider>
          {:else if error}
            <div class="space-y-2">
              <div class="inline-flex items-center gap-2 text-xs font-medium text-error">
                <span class="px-2 py-0.5 rounded bg-error/10">{stageLabels[error.stage]}</span>
                校验未通过
              </div>
              <pre class="whitespace-pre-wrap break-words text-xs text-base-content/70">{error.message}</pre>
            </div>
          {/if}
        </div>
      </div>
    </div>

    {#if !isEdit}
      <section class="mt-6">
        <div class="mb-2 text-xs text-base-content/60">
          从示例开始 · 点击卡片载入到左侧编辑（{examplePreviews.length}）
        </div>
        <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
          {#each examplePreviews as ex (ex.id)}
            <button
              type="button"
              onclick={() => loadExample(ex.id)}
              title={ex.description}
              class="group flex flex-col overflow-hidden rounded-lg border bg-base-100 text-left transition hover:shadow-sm {loadedExampleId ===
              ex.id
                ? 'border-primary ring-1 ring-primary'
                : 'border-base-300 hover:border-primary/60'}"
            >
              <div class="border-b border-base-300 px-3 py-2">
                <div class="flex items-center justify-between gap-2">
                  <span class="truncate text-sm font-medium text-base-content">{ex.name}</span>
                  {#if loadedExampleId === ex.id}
                    <span class="shrink-0 text-[10px] font-medium text-primary">已载入</span>
                  {/if}
                </div>
                <div class="mt-0.5 line-clamp-2 text-xs text-base-content/55">{ex.description}</div>
              </div>
              <div class="relative h-40 overflow-hidden bg-base-200/30 p-3">
                {#if ex.preview}
                  <div class="pointer-events-none">
                    <JsonUIProvider initialState={{}}>
                      <Renderer spec={ex.preview} registry={uiRegistry} />
                    </JsonUIProvider>
                  </div>
                  <div
                    class="pointer-events-none absolute inset-x-0 bottom-0 h-10 bg-gradient-to-t from-base-100 to-transparent"
                  ></div>
                {:else}
                  <div class="text-xs text-base-content/40">预览不可用</div>
                {/if}
              </div>
            </button>
          {/each}
        </div>
      </section>
    {/if}

    <details class="mt-4 text-xs text-base-content/60">
      <summary class="cursor-pointer select-none">可用组件（{catalogComponents.length}）</summary>
      <ul class="mt-2 space-y-1">
        {#each catalogComponents as component (component.name)}
          <li>
            <span class="font-mono text-base-content/80">{component.name}</span> — {component.description}
          </li>
        {/each}
      </ul>
    </details>
  </div>
</div>

<ConfirmModal
  title="删除 GenUI"
  message="确认要删除这份 GenUI 吗？引用它的 Agent 将自动解除关联。此操作不可撤销。"
  confirmText="删除"
  confirmButtonStyle="danger"
  open={showDeleteConfirm}
  onClose={() => (showDeleteConfirm = false)}
  onConfirm={handleDelete}
/>
