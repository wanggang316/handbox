<script lang="ts">
  /**
   * Tool detail.
   *
   * One route serves three sources, because none of their ids can collide:
   * a custom tool's uuid (editable), a built-in registration name, and an
   * `mcp__<serverId>__<tool>` name (both read-only). `new` opens the editor on
   * a blank definition.
   *
   * The editable case is a contract in two halves — the parameters the model
   * fills in, and the view that renders them — so the page validates them
   * TOGETHER before saving: a binding to a parameter nobody declared would
   * otherwise only surface as a blank card mid-conversation.
   */
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { Trash2, ExternalLink } from "@lucide/svelte";
  import { Renderer, JsonUIProvider } from "@json-render/svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Input from "$lib/components/ui/Input.svelte";
  import Textarea from "$lib/components/ui/Textarea.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import Spinner from "$lib/components/ui/Spinner.svelte";
  import IconPicker from "$lib/components/ui/IconPicker.svelte";
  import ConfirmModal from "$lib/components/ui/ConfirmModal.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import TableBaseRow from "$lib/components/ui/table/TableBaseRow.svelte";
  import ToolParamsEditor from "$lib/components/settings/ToolParamsEditor.svelte";
  import DetailHeader from "$lib/components/settings/DetailHeader.svelte";
  import { uiRegistry } from "$lib/components/genui/jsonui/registry";
  import { explainSpec } from "$lib/components/genui/jsonui/resolveSpec";
  import {
    sampleStateFromParams,
    validateBindings,
  } from "$lib/components/genui/bindings";
  import { getAgentToolCatalog } from "$lib/api/toolDefinition";
  import {
    toolDefinitionState,
    toolDefinitionActions,
  } from "$lib/states/toolDefinition.svelte";
  import { genuiState, genuiActions } from "$lib/states/genui.svelte";
  import { mcpState, mcpActions } from "$lib/states/mcp.svelte";
  import { resolveToolIcon } from "$lib/constants/agentTools";
  import { resolveAgentIcon } from "$lib/utils/agentIcons";
  import { renderCodeBlock } from "$lib/utils/code";
  import { normalizeError } from "$lib/utils/error";
  import { parseMcpToolName } from "$lib/utils/toolCall";
  import { t } from "$lib/i18n";
  import type {
    BuiltinToolInfo,
    ToolParam,
  } from "$lib/types/toolDefinition";

  const toolId = $derived($page.params.toolId ?? "");
  const isCreating = $derived(toolId === "new");

  let loading = $state(true);
  let saving = $state(false);
  let saveError = $state<string | null>(null);
  let deleteOpen = $state(false);
  let builtinCatalog = $state<BuiltinToolInfo[]>([]);

  // Editable form. Seeded from the stored definition once it resolves; the
  // create case starts blank.
  let form = $state({
    name: "",
    displayName: "",
    icon: "",
    description: "",
    parameters: [] as ToolParam[],
    genuiId: "",
  });
  /** Guards the seed so typing is not overwritten when the store re-emits. */
  let seededFor = $state<string | null>(null);

  const custom = $derived(
    isCreating
      ? undefined
      : toolDefinitionState.tools.find((tool) => tool.id === toolId),
  );
  const builtin = $derived(
    builtinCatalog.find((tool) => tool.name === toolId),
  );
  const mcpName = $derived(parseMcpToolName(toolId));
  const mcpTool = $derived.by(() => {
    if (!mcpName) return null;
    const server = mcpState.servers.find((s) => s.id === mcpName.serverId);
    const tool = server?.tools.find((item) => item.name === mcpName.tool);
    return server && tool ? { server, tool } : null;
  });

  const editable = $derived(isCreating || custom !== undefined);

  onMount(async () => {
    const loads: Promise<unknown>[] = [
      toolDefinitionActions.loadTools().catch((error) => {
        console.error("加载自定义工具失败:", error);
      }),
      genuiActions.loadGenuis().catch((error) => {
        console.error("加载 GenUI 列表失败:", error);
      }),
      getAgentToolCatalog()
        .then((catalog) => {
          builtinCatalog = catalog;
        })
        .catch((error) => {
          console.error("加载内置工具目录失败:", error);
        }),
    ];
    // Only an `mcp__…` id needs the server list, and loading it can hit the
    // servers themselves — not worth doing for every tool detail.
    if (parseMcpToolName(toolId)) {
      loads.push(
        mcpActions.loadServers(mcpState.needsRefresh).catch((error) => {
          console.error("加载 MCP 服务器失败:", error);
        }),
      );
    }
    await Promise.all(loads);
    loading = false;
  });

  // Seed the form once per resolved definition — including after a create,
  // where the route changes to the new id.
  $effect(() => {
    if (isCreating) {
      if (seededFor !== "new") {
        form = {
          name: "",
          displayName: "",
          icon: "",
          description: "",
          parameters: [],
          genuiId: "",
        };
        seededFor = "new";
      }
      return;
    }
    if (!custom || seededFor === custom.id) return;
    form = {
      name: custom.name,
      displayName: custom.displayName,
      icon: custom.icon ?? "",
      description: custom.description,
      parameters: custom.parameters.map((param) => ({ ...param })),
      genuiId: custom.genuiId ?? "",
    };
    seededFor = custom.id;
  });

  const genuiOptions = $derived([
    { value: "", label: t("settings.tools.detail.viewNone") },
    ...genuiState.genuis.map((genui) => ({
      value: genui.id ?? "",
      label: genui.name,
    })),
  ]);

  const linkedGenui = $derived(
    form.genuiId
      ? genuiState.genuis.find((genui) => genui.id === form.genuiId)
      : undefined,
  );

  /**
   * The linked spec, validated with bindings allowed. `explainSpec` rather than
   * `resolveSpec` so an unrenderable view says WHY on the form instead of
   * silently previewing nothing.
   */
  const specDiagnostic = $derived(
    linkedGenui ? explainSpec(linkedGenui.spec, { allowBindings: true }) : null,
  );

  /** Bindings that name a parameter the tool does not declare. */
  const bindingIssues = $derived(
    specDiagnostic?.ok
      ? validateBindings(specDiagnostic.spec, form.parameters)
      : [],
  );

  const previewState = $derived(sampleStateFromParams(form.parameters));

  const canSave = $derived(
    !saving &&
      form.name.trim().length > 0 &&
      bindingIssues.length === 0 &&
      (specDiagnostic === null || specDiagnostic.ok),
  );

  function back(): void {
    void goto("/settings/agent-tools");
  }

  async function save(): Promise<void> {
    saving = true;
    saveError = null;
    try {
      const payload = {
        name: form.name.trim(),
        displayName: form.displayName.trim() || form.name.trim(),
        icon: form.icon,
        description: form.description,
        // Snapshot: the parameters cross an IPC boundary, and `$state` hands
        // out proxies that must not be serialized as-is.
        parameters: $state.snapshot(form.parameters),
        genuiId: form.genuiId,
      };
      if (isCreating) {
        const created = await toolDefinitionActions.createTool(payload);
        // Stay on the tool, now in edit mode, rather than bouncing to the list.
        seededFor = created.id;
        await goto(`/settings/agent-tools/${created.id}`, {
          replaceState: true,
        });
      } else {
        await toolDefinitionActions.updateTool(toolId, payload);
      }
    } catch (error) {
      saveError = normalizeError(
        error,
        t("settings.tools.detail.saveFailed"),
      ).message;
    } finally {
      saving = false;
    }
  }

  async function confirmDelete(): Promise<void> {
    try {
      await toolDefinitionActions.deleteTool(toolId);
      back();
    } catch (error) {
      saveError = normalizeError(
        error,
        t("settings.tools.detail.saveFailed"),
      ).message;
    } finally {
      deleteOpen = false;
    }
  }

  function openGenui(): void {
    void goto(form.genuiId ? `/genui/${form.genuiId}` : "/genui/new");
  }

  function renderSchema(parameters: unknown): string {
    return renderCodeBlock(JSON.stringify(parameters ?? {}, null, 2), {
      language: "json",
      variant: "compact",
    });
  }

  // Read-only header fields, whichever source resolved.
  const readOnlyTool = $derived.by(() => {
    if (builtin) {
      return {
        name: builtin.name,
        label: builtin.label || builtin.name,
        description: builtin.description,
        parameters: builtin.parameters,
        note: t("settings.tools.detail.readOnly"),
        Icon: resolveToolIcon(builtin.name),
      };
    }
    if (mcpTool) {
      return {
        name: mcpTool.tool.name,
        label: mcpTool.server.displayName || mcpTool.server.name,
        description: mcpTool.tool.description ?? "",
        parameters: mcpTool.tool.inputSchema,
        note: t("settings.tools.detail.mcpReadOnly"),
        Icon: resolveToolIcon(toolId),
      };
    }
    return null;
  });

  /**
   * What the sticky header says. It follows the display name as it is typed —
   * the header is the only place the record is named once the page scrolls, so
   * it has to agree with the field the reader is editing.
   */
  const headerTitle = $derived.by(() => {
    if (loading) return t("settings.tools.detail.newTitle");
    if (editable) {
      const named = form.displayName.trim() || form.name.trim();
      return named || t("settings.tools.detail.newTitle");
    }
    return readOnlyTool?.label ?? t("settings.tools.detail.notFound");
  });
</script>

<!-- The page does NOT own a scroller: the settings layout is the scroll
     container, and nesting one here is what let the header scroll out of
     reach. It stays put because `DetailHeader` is sticky. -->
{#snippet editorActions()}
  <Button variant="primary" disabled={!canSave} onclick={save}>
    {t("common.save")}
  </Button>
  {#if !isCreating}
    <Button
      variant="clear"
      size="icon"
      ariaLabel={t("settings.tools.detail.deleteTitle")}
      class="text-base-content/40 enabled:hover:text-error"
      onclick={() => (deleteOpen = true)}
    >
      <Trash2 size={18} />
    </Button>
  {/if}
{/snippet}

<div class="flex flex-col">
  <DetailHeader
    title={headerTitle}
    backLabel={t("settings.tools.detail.back")}
    onBack={back}
    actions={editable ? editorActions : undefined}
  />

  <div class="px-6 pr-8 pb-10">
    {#if loading}
      <div class="flex justify-center py-16">
        <Spinner />
      </div>
    {:else if editable}
      <div class="flex flex-col gap-y-4">
        <div class="flex items-center gap-3">
          <IconPicker
            value={form.icon}
            label={t("settings.tools.detail.basics")}
            onSelect={(name) => (form.icon = name)}
          />
          <div class="min-w-0 flex-1">
            <Input
              value={form.displayName}
              placeholder={t("settings.tools.detail.displayNamePlaceholder")}
              onInput={(value) => (form.displayName = value)}
            />
          </div>
        </div>

        {#if saveError}
          <p class="text-sm text-error">{saveError}</p>
        {/if}

        <TableGroup>
          <TableBaseRow
            label={t("settings.tools.detail.name")}
            layout="vertical"
            helpText={t("settings.tools.detail.nameHint")}
          >
            <Input
              value={form.name}
              placeholder={t("settings.tools.detail.namePlaceholder")}
              literal
              onInput={(value) => (form.name = value)}
            />
          </TableBaseRow>

          <TableBaseRow
            label={t("settings.tools.detail.prompt")}
            layout="vertical"
            helpText={t("settings.tools.detail.promptHint")}
          >
            <Textarea
              bind:value={form.description}
              rows={4}
              placeholder={t("settings.tools.detail.promptPlaceholder")}
            />
          </TableBaseRow>
        </TableGroup>

        <TableGroup>
          <TableBaseRow
            layout="vertical"
            label={t("settings.tools.detail.params")}
            helpText={t("settings.tools.detail.paramsHint")}
          >
            <ToolParamsEditor
              parameters={form.parameters}
              onChange={(parameters) => (form.parameters = parameters)}
            />
          </TableBaseRow>
        </TableGroup>

        <TableGroup>
          <TableBaseRow
            layout="vertical"
            label={t("settings.tools.detail.view")}
            helpText={t("settings.tools.detail.viewHint")}
          >
            <div class="flex items-center gap-3">
              <div class="min-w-0 flex-1">
                <Select
                  value={form.genuiId}
                  options={genuiOptions}
                  onChange={(value) => (form.genuiId = value)}
                />
              </div>
              <Button variant="secondary" size="sm" onclick={openGenui}>
                <ExternalLink size={14} />
                {form.genuiId
                  ? t("settings.tools.detail.viewEdit")
                  : t("settings.tools.detail.viewNew")}
              </Button>
            </div>

            <!-- Everything that can be wrong about a view, said here rather
                 than discovered in a conversation. -->
            {#if form.genuiId && !linkedGenui}
              <p class="mt-2 text-sm text-warning">
                {t("settings.tools.detail.previewMissing")}
              </p>
            {:else if specDiagnostic && !specDiagnostic.ok}
              <p class="mt-2 text-sm text-error">
                {t("settings.tools.detail.previewInvalid", {
                  reason: specDiagnostic.message,
                })}
              </p>
            {:else if bindingIssues.length > 0}
              <ul class="mt-2 flex flex-col gap-1">
                {#each bindingIssues as issue (issue.binding.elementId + issue.binding.prop)}
                  <li class="text-sm text-error">{issue.message}</li>
                {/each}
              </ul>
            {:else if specDiagnostic?.ok}
              <div class="mt-3 flex flex-col gap-2">
                <p class="text-[13px] text-base-content/55">
                  {t("settings.tools.detail.previewHint")}
                </p>
                <div
                  class="rounded-lg border border-[var(--hairline)] bg-base-100 p-3"
                >
                  <JsonUIProvider initialState={previewState}>
                    <Renderer spec={specDiagnostic.spec} registry={uiRegistry} />
                  </JsonUIProvider>
                </div>
              </div>
            {/if}
          </TableBaseRow>
        </TableGroup>
      </div>
    {:else if readOnlyTool}
      {@const ReadOnlyIcon = readOnlyTool.Icon}
      <div class="flex flex-col gap-y-4">
        <div class="flex items-center gap-3">
          <div
            class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-base-200 text-base-content/70"
          >
            <ReadOnlyIcon size={20} />
          </div>
          <div class="min-w-0">
            <p class="truncate text-sm font-medium text-base-content">
              {readOnlyTool.label}
            </p>
            <p class="truncate font-mono text-xs text-base-content/50">
              {readOnlyTool.name}
            </p>
          </div>
          <span class="ml-auto shrink-0 text-xs text-base-content/50">
            {readOnlyTool.note}
          </span>
        </div>

        <TableGroup>
          <TableBaseRow
            label={t("settings.tools.detail.prompt")}
            layout="vertical"
          >
            <p
              class="whitespace-pre-wrap text-[13px] leading-relaxed text-base-content/70"
            >
              {readOnlyTool.description}
            </p>
          </TableBaseRow>

          <TableBaseRow
            label={t("settings.tools.detail.schema")}
            layout="vertical"
          >
            <div class="max-h-96 overflow-auto text-[11px]">
              {@html renderSchema(readOnlyTool.parameters)}
            </div>
          </TableBaseRow>
        </TableGroup>
      </div>
    {:else}
      <p class="py-16 text-center text-sm text-base-content/55">
        {t("settings.tools.detail.notFound")}
      </p>
    {/if}
  </div>
</div>

<ConfirmModal
  open={deleteOpen}
  title={t("settings.tools.detail.deleteTitle")}
  message={t("settings.tools.detail.deleteMessage", {
    name: form.displayName || form.name,
  })}
  onConfirm={confirmDelete}
  onClose={() => (deleteOpen = false)}
/>
