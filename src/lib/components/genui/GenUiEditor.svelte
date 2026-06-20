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
  import Select from "$lib/components/ui/Select.svelte";
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

  // 「载入示例」选择器：从内置模板库填充编辑器（新建模式）。选择后即把 spec 写入
  // 左侧文本域、空名称时顺带填名，再把选择器复位到首项（值为空、非 disabled，故
  // 能真正回弹，也允许再次选中同一示例以「重置」编辑）。
  let exampleChoice = $state("");
  const exampleOptions = [
    { value: "", label: "从示例载入…" },
    ...genuiExamples.map((e) => ({ value: e.id, label: e.name })),
  ];

  function loadExample(id: string) {
    const ex = genuiExamples.find((e) => e.id === id);
    if (!ex) return;
    specInput = JSON.stringify(ex.spec, null, 2);
    if (!name.trim()) name = ex.name;
    // 复位到首项要放到下一个 tick：在 change 事件同一刷新内重置受控 <select> 不会
    // 回写到 DOM。延迟一拍让 Svelte 重新同步原生元素，回到「从示例载入…」。
    setTimeout(() => {
      exampleChoice = "";
    }, 0);
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
        {#if !isEdit}
          <Select
            options={exampleOptions}
            bind:selectedValue={exampleChoice}
            onChange={loadExample}
            size="sm"
            autoWidth
          />
        {/if}
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
    <div class="grid gap-4 lg:grid-cols-2 h-full min-h-0">
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
