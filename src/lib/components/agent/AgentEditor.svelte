<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { ArrowLeft, Check, Save, Search } from "@lucide/svelte";
  import Button from "../ui/Button.svelte";
  import Select from "../ui/Select.svelte";
  import LabeledSlider from "../ui/LabeledSlider.svelte";
  import Modal from "../ui/Modal.svelte";
  import {
    TableGroup,
    TableBaseRow,
    SelectRow,
    SwitchRow,
  } from "../ui/table";
  import DefaultRow from "../ui/table/DefaultRow.svelte";
  import { AGENT_ICONS, resolveAgentIcon } from "$lib/utils/agentIcons";
  import { normalizeError } from "$lib/utils/error";
  import { t } from "$lib/i18n";
  import type { Agent } from "$lib/types";
  import type { McpServerConfig } from "$lib/types/llm";
  import { agentActions } from "$lib/states/agent.svelte";
  import { mcpState, mcpActions } from "$lib/states/mcp.svelte";
  import { genuiState, genuiActions } from "$lib/states/genui.svelte";
  import { listSkills } from "$lib/api/skill";
  import type { SkillInfo } from "$lib/types";
  import { BUILTIN_TOOL_IDS } from "$lib/constants/builtinToolIds";

  interface Props {
    // Existing agent for edit mode; null for create mode.
    agent?: Agent | null;
  }

  interface AgentFormData {
    name: string;
    // Lucide kebab-case icon name; empty string means the default icon.
    icon: string;
    temperature?: number;
    maxTokens?: number;
    systemPrompt: string;
    // Linked skill names (referenced by name; injected every turn at runtime).
    skills: string[];
    mcpServers: McpServerConfig[];
    generativeUi: boolean;
    // Linked GenUI id; empty string means none.
    genuiId: string;
    description: string;
    builtinTools: string[];
    workingDirMode: string;
    toolExecutionMode: string;
  }

  let { agent = null }: Props = $props();

  // Only temperature / maxTokens: top_p / top_k are consumed by neither the
  // session layer nor the engine, so they are not exposed here.
  type ParamKey = "temperature" | "maxTokens";
  const PARAM_META: Array<{
    key: ParamKey;
    label: string;
    min: number;
    max: number;
    step: number;
    default: number;
  }> = [
    { key: "temperature", label: "Temperature", min: 0, max: 2, step: 0.1, default: 0.7 },
    { key: "maxTokens", label: "Max Tokens", min: 256, max: 16384, step: 256, default: 4096 },
  ];

  const genuiOptions = $derived([
    { value: "", label: t("agent.form.genuiNone") },
    ...genuiState.genuis.map((g) => ({ value: g.id ?? "", label: g.name })),
  ]);

  const executionModeOptions = $derived([
    { value: "auto", label: t("agent.input.autoExecution") },
    { value: "manual", label: t("agent.input.manualExecution") },
  ]);

  // Builtin tool names (canonical source constants/builtinToolIds, matching backend values).
  const BUILTIN_TOOLS = BUILTIN_TOOL_IDS;
  // $derived so labels track language switch.
  const workingDirModeOptions = $derived([
    { value: "required", label: t("agent.form.workingDirRequired") },
    { value: "optional", label: t("agent.form.workingDirOptional") },
    { value: "none", label: t("agent.form.workingDirNone") },
  ]);
  const toolExecutionModeOptions = $derived([
    { value: "auto", label: t("agent.input.autoExecution") },
    { value: "manual", label: t("agent.input.manualExecution") },
  ]);

  // Builtin agents: the name is read-only (enforced by the backend).
  const isBuiltin = $derived(agent?.builtin ?? false);

  function isToolSelected(tool: string): boolean {
    return formData.builtinTools.includes(tool);
  }
  function toggleBuiltinTool(tool: string, selected: boolean) {
    if (selected) {
      if (!formData.builtinTools.includes(tool)) {
        formData.builtinTools = [...formData.builtinTools, tool];
      }
    } else {
      formData.builtinTools = formData.builtinTools.filter((x) => x !== tool);
    }
  }

  const availableServers = $derived(
    mcpState.servers.filter(
      (s) => s.enabled && s.status === "ready" && s.enabledTools.length > 0
    )
  );

  // Definition-level linking is project-agnostic: no workingDir passed
  // (user/appData tiers only), only valid skills listed. Linked names missing
  // on disk stay visible for unlinking; unknown names are skipped at runtime.
  let availableSkills = $state<SkillInfo[]>([]);

  // Skill / MCP selection happens in modals; the rows only show linked counts.
  let skillsModalOpen = $state(false);
  let mcpModalOpen = $state(false);

  // Icon picker popover: picking replaces and closes; picking the current icon
  // clears it (back to the default Bot). Outside click closes.
  let iconPickerOpen = $state(false);

  function handleIconPickerOutside(event: MouseEvent) {
    if (!iconPickerOpen) return;
    const target = event.target as HTMLElement;
    if (!target.closest(".icon-picker")) {
      iconPickerOpen = false;
    }
  }

  function pickIcon(name: string) {
    formData.icon = formData.icon === name ? "" : name;
    iconPickerOpen = false;
  }

  let skillSearch = $state("");
  const filteredSkills = $derived.by(() => {
    const q = skillSearch.trim().toLowerCase();
    if (!q) return availableSkills;
    return availableSkills.filter(
      (s) =>
        s.name.toLowerCase().includes(q) ||
        (s.description ?? "").toLowerCase().includes(q)
    );
  });

  function isSkillSelected(name: string): boolean {
    return formData.skills.includes(name);
  }
  function toggleSkill(name: string, selected: boolean) {
    if (selected) {
      if (!formData.skills.includes(name)) {
        formData.skills = [...formData.skills, name];
      }
    } else {
      formData.skills = formData.skills.filter((x) => x !== name);
    }
  }

  onMount(() => {
    if (genuiState.genuis.length === 0) {
      genuiActions
        .loadGenuis()
        .catch((e) => console.error("Failed to load GenUIs:", e));
    }
    if (!mcpState.initialized) {
      mcpActions
        .loadServers()
        .catch((e) => console.error("Failed to load MCP servers:", e));
    }
    listSkills()
      .then((skills) => {
        availableSkills = skills.filter((s) => s.body !== null);
      })
      .catch((e) => console.error("Failed to list skills:", e));
  });

  let formData = $state<AgentFormData>({
    name: "",
    icon: "",
    systemPrompt: "",
    skills: [],
    mcpServers: [],
    generativeUi: false,
    genuiId: "",
    description: "",
    builtinTools: [],
    workingDirMode: "optional",
    toolExecutionMode: "auto",
  });

  const CurrentIcon = $derived(resolveAgentIcon(formData.icon));

  // Linked names absent from discovery (skill deleted/renamed): kept as removable rows.
  const missingSelectedSkills = $derived(
    formData.skills.filter(
      (name) => !availableSkills.some((s) => s.name === name)
    )
  );

  let paramEnabled = $state<Record<ParamKey, boolean>>({
    temperature: false,
    maxTokens: false,
  });
  let paramValues = $state<Record<ParamKey, number>>({
    temperature: 0.7,
    maxTokens: 4096,
  });

  let saving = $state(false);

  function isMcpSelected(serverId: string): boolean {
    return formData.mcpServers.some((s) => s.serverId === serverId);
  }
  function mcpMode(serverId: string): "auto" | "manual" {
    return (
      formData.mcpServers.find((s) => s.serverId === serverId)?.executionMode ??
      "auto"
    );
  }
  function toggleMcp(serverId: string, selected: boolean) {
    if (selected) {
      if (!formData.mcpServers.some((s) => s.serverId === serverId)) {
        const server = mcpState.servers.find((s) => s.id === serverId);
        formData.mcpServers = [
          ...formData.mcpServers,
          {
            serverId,
            executionMode: "auto",
            enabledTools: server?.enabledTools ?? [],
          },
        ];
      }
    } else {
      formData.mcpServers = formData.mcpServers.filter(
        (s) => s.serverId !== serverId
      );
    }
  }
  function setMcpMode(serverId: string, mode: "auto" | "manual") {
    formData.mcpServers = formData.mcpServers.map((s) =>
      s.serverId === serverId ? { ...s, executionMode: mode } : s
    );
  }

  function backToList() {
    goto("/agents");
  }

  /** Persist the form: edit diffs field-by-field; create writes non-default capability fields afterwards. */
  async function persist(data: AgentFormData) {
    // The linked GenUI only applies while generative UI is on; clear it otherwise.
    const effectiveGenuiId =
      data.generativeUi && data.genuiId ? data.genuiId : null;

    if (agent?.id) {
      // Only write the name when it actually changed: the backend rejects
      // renaming builtin agents, so an unconditional write would fail edits
      // that only touch other fields.
      if (data.name !== agent.name) {
        await agentActions.updateAgentName(agent.id, data.name);
      }

      // Normalize an empty icon to null (clears the custom icon).
      if ((data.icon || null) !== (agent.icon ?? null)) {
        await agentActions.updateAgentField(agent.id, "icon", data.icon || null);
      }

      const hasChanged = <T,>(a: T | undefined, b: T | undefined) =>
        a !== b && !(a === undefined && b === undefined);

      if (hasChanged(data.temperature, agent.temperature)) {
        await agentActions.updateAgentField(
          agent.id,
          "temperature",
          data.temperature ?? null
        );
      }
      if (hasChanged(data.maxTokens, agent.maxTokens)) {
        await agentActions.updateAgentField(
          agent.id,
          "maxTokens",
          data.maxTokens ?? null
        );
      }
      if (data.systemPrompt !== agent.systemPrompt) {
        await agentActions.updateAgentField(
          agent.id,
          "systemPrompt",
          data.systemPrompt || null
        );
      }

      if (
        JSON.stringify(data.mcpServers ?? []) !==
        JSON.stringify(agent.mcpServers ?? [])
      ) {
        await agentActions.updateAgentField(
          agent.id,
          "mcpServers",
          data.mcpServers
        );
      }

      // Compare booleans explicitly: turning it off must send false, not be skipped as falsy.
      if ((data.generativeUi ?? false) !== (agent.generativeUi ?? false)) {
        await agentActions.updateAgentField(
          agent.id,
          "generativeUi",
          data.generativeUi ?? false
        );
      }

      // null unlinks the GenUI.
      if ((agent.genuiId ?? null) !== effectiveGenuiId) {
        await agentActions.updateAgentField(agent.id, "genuiId", effectiveGenuiId);
      }

      if (
        JSON.stringify(data.skills ?? []) !== JSON.stringify(agent.skills ?? [])
      ) {
        await agentActions.updateAgentField(agent.id, "skills", data.skills);
      }

      // Capability fields: the backend only supports per-field updates.
      if (data.description !== (agent.description ?? "")) {
        await agentActions.updateAgentField(
          agent.id,
          "description",
          data.description || null
        );
      }
      if (
        JSON.stringify(data.builtinTools ?? []) !==
        JSON.stringify(agent.builtinTools ?? [])
      ) {
        await agentActions.updateAgentField(
          agent.id,
          "builtinTools",
          data.builtinTools
        );
      }
      if (data.workingDirMode !== (agent.workingDirMode ?? "optional")) {
        await agentActions.updateAgentField(
          agent.id,
          "workingDirMode",
          data.workingDirMode
        );
      }
      if (data.toolExecutionMode !== (agent.toolExecutionMode ?? "auto")) {
        await agentActions.updateAgentField(
          agent.id,
          "toolExecutionMode",
          data.toolExecutionMode
        );
      }
    } else {
      // Create does not accept capability fields; write them after creation.
      const newAgent = await agentActions.createAgent({
        name: data.name,
        temperature: data.temperature,
        maxTokens: data.maxTokens,
        systemPrompt: data.systemPrompt || undefined,
        reasoning: undefined,
        mcpServers: data.mcpServers,
        skills: data.skills,
        generativeUi: data.generativeUi,
        genuiId: effectiveGenuiId ?? undefined,
      });

      // Create-then-update only for non-default capability fields.
      if (newAgent.id) {
        if (data.icon) {
          await agentActions.updateAgentField(newAgent.id, "icon", data.icon);
        }
        if (data.description) {
          await agentActions.updateAgentField(
            newAgent.id,
            "description",
            data.description
          );
        }
        if (data.builtinTools.length > 0) {
          await agentActions.updateAgentField(
            newAgent.id,
            "builtinTools",
            data.builtinTools
          );
        }
        if (data.workingDirMode !== "optional") {
          await agentActions.updateAgentField(
            newAgent.id,
            "workingDirMode",
            data.workingDirMode
          );
        }
        if (data.toolExecutionMode !== "auto") {
          await agentActions.updateAgentField(
            newAgent.id,
            "toolExecutionMode",
            data.toolExecutionMode
          );
        }
      }
    }
  }

  async function handleSave() {
    if (!formData.name.trim() || saving) {
      return;
    }
    formData.temperature = paramEnabled.temperature
      ? paramValues.temperature
      : undefined;
    formData.maxTokens = paramEnabled.maxTokens
      ? paramValues.maxTokens
      : undefined;

    saving = true;
    try {
      await persist(formData);
      backToList();
    } catch (error) {
      console.error("Failed to save agent:", error);
      const normalized = normalizeError(error, t("agent.form.saveFailed"));
      alert(`${t("agent.form.saveFailed")}\n${normalized.hint ?? normalized.message}`);
    } finally {
      saving = false;
    }
  }

  $effect(() => {
    if (agent) {
      formData = {
        name: agent.name,
        icon: agent.icon ?? "",
        temperature: agent.temperature,
        maxTokens: agent.maxTokens,
        systemPrompt: agent.systemPrompt || "",
        skills: agent.skills ? [...agent.skills] : [],
        mcpServers: agent.mcpServers ? [...agent.mcpServers] : [],
        generativeUi: agent.generativeUi ?? false,
        genuiId: agent.genuiId ?? "",
        description: agent.description ?? "",
        builtinTools: agent.builtinTools ? [...agent.builtinTools] : [],
        workingDirMode: agent.workingDirMode ?? "optional",
        toolExecutionMode: agent.toolExecutionMode ?? "auto",
      };
    } else {
      formData = {
        name: "",
        icon: "",
        systemPrompt: "",
        skills: [],
        mcpServers: [],
        generativeUi: false,
        genuiId: "",
        description: "",
        builtinTools: [],
        workingDirMode: "optional",
        toolExecutionMode: "auto",
      };
    }

    const source: Record<ParamKey, number | undefined | null> = {
      temperature: agent?.temperature,
      maxTokens: agent?.maxTokens,
    };
    for (const p of PARAM_META) {
      const v = source[p.key];
      const has = v !== undefined && v !== null;
      paramEnabled[p.key] = has;
      paramValues[p.key] = has ? (v as number) : p.default;
    }
  });
</script>

<!-- Agent editor page in the settings style: centered max-w-3xl reading width,
     TableGroup cards; Skill / MCP picked via modals. -->
<div class="h-full flex flex-col">
  <div class="flex-shrink-0 px-6 pb-4 pt-12">
    <div class="mx-auto w-full max-w-3xl">
      <button
        class="flex items-center gap-2 text-sm text-base-content/70 hover:text-base-content w-fit mb-4"
        onclick={backToList}
      >
        <ArrowLeft size={14} />
        {t("agent.form.backToList")}
      </button>

      <div class="flex items-center gap-3">
        <!-- Current-icon button; click opens the in-place picker popover. -->
        <div class="icon-picker relative flex-shrink-0">
          <button
            type="button"
            aria-expanded={iconPickerOpen}
            title={t("agent.form.iconLabel")}
            class="flex h-10 w-10 items-center justify-center rounded-lg bg-base-200 text-base-content/70 transition-colors hover:bg-base-300 hover:text-base-content"
            onclick={() => (iconPickerOpen = !iconPickerOpen)}
          >
            <CurrentIcon size={20} />
          </button>
          {#if iconPickerOpen}
            <div
              class="absolute left-0 top-full z-[var(--z-popover)] mt-2 w-[19rem] rounded-xl border border-[var(--hairline)] bg-[var(--bg-card)] p-3 shadow-xl"
            >
              <div class="flex flex-wrap gap-1.5">
                {#each AGENT_ICONS as opt (opt.name)}
                  {@const Icon = opt.Icon}
                  <button
                    type="button"
                    aria-pressed={formData.icon === opt.name}
                    title={opt.name}
                    class="flex h-8 w-8 items-center justify-center rounded-md border transition-colors {formData.icon ===
                    opt.name
                      ? 'border-primary/40 bg-primary/10 text-primary'
                      : 'border-transparent text-base-content/55 hover:bg-base-200 hover:text-base-content'}"
                    onclick={() => pickIcon(opt.name)}
                  >
                    <Icon size={16} />
                  </button>
                {/each}
              </div>
            </div>
          {/if}
        </div>

        <div class="min-w-0 flex-1">
          <input
            class="modal-title-input w-full"
            bind:value={formData.name}
            placeholder={t("agent.form.namePlaceholder")}
            disabled={isBuiltin}
          />
        </div>
        <Button
          variant="primary"
          size="sm"
          onclick={handleSave}
          disabled={saving || !formData.name.trim()}
          customClass="flex items-center gap-2"
        >
          <Save size={14} />
          {saving
            ? t("common.saving")
            : agent
              ? t("common.save")
              : t("common.create")}
        </Button>
      </div>
    </div>
  </div>

  <div class="flex-1 min-h-0 overflow-y-auto px-6 pb-6">
    <div class="mx-auto flex w-full max-w-3xl flex-col gap-y-4">

      <TableGroup title={t("agent.form.systemPromptTitle")}>
        <TableBaseRow>
          <textarea
            class="field min-h-48 w-full resize-y px-3 py-2.5 font-mono text-sm leading-relaxed"
            bind:value={formData.systemPrompt}
            placeholder={t("agent.systemPrompt.placeholder")}
          ></textarea>
          <div class="mt-1 text-right text-xs text-base-content/35">
            {t("agent.form.charCount", { count: formData.systemPrompt.length })}
          </div>
        </TableBaseRow>
      </TableGroup>

      <TableGroup title={t("agent.form.sectionTools")}>
        <TableBaseRow label={t("agent.form.builtinTools")} layout="vertical">
          <div class="flex flex-wrap gap-1.5">
            {#each BUILTIN_TOOLS as tool (tool)}
              <button
                type="button"
                aria-pressed={isToolSelected(tool)}
                class="rounded-md border px-2 py-1 font-mono text-xs transition-colors {isToolSelected(
                  tool,
                )
                  ? 'border-primary/40 bg-primary/10 text-primary'
                  : 'border-[var(--hairline)] text-base-content/60 hover:border-[var(--hairline-strong)] hover:text-base-content'}"
                onclick={() => toggleBuiltinTool(tool, !isToolSelected(tool))}
              >
                {tool}
              </button>
            {/each}
          </div>
        </TableBaseRow>

        <SelectRow
          label={t("agent.form.toolExecution")}
          options={toolExecutionModeOptions}
          bind:selectedValue={formData.toolExecutionMode}
        />

        <DefaultRow
          label={t("agent.form.skillsTitle")}
          value={t("agent.form.linkedCount", { count: formData.skills.length })}
          onclick={() => (skillsModalOpen = true)}
        />

        <DefaultRow
          label={t("agent.form.mcpServers")}
          value={t("agent.form.linkedCount", {
            count: formData.mcpServers.length,
          })}
          onclick={() => (mcpModalOpen = true)}
        />
      </TableGroup>

      <TableGroup title={t("agent.form.sectionRuntime")}>
        <SelectRow
          label={t("agent.form.workingDir")}
          options={workingDirModeOptions}
          bind:selectedValue={formData.workingDirMode}
        />

        <SwitchRow
          label={t("agent.form.generativeUi")}
          description={t("agent.form.generativeUiDesc")}
          bind:checked={formData.generativeUi}
        />

        {#if formData.generativeUi}
          <SelectRow
            label={t("agent.form.genuiHint")}
            options={genuiOptions}
            bind:selectedValue={formData.genuiId}
          />
        {/if}
      </TableGroup>

      <TableGroup
        title={t("agent.form.modelParams")}
        collapsible
        defaultCollapsed
      >
        {#each PARAM_META as p (p.key)}
          <SwitchRow label={p.label} bind:checked={paramEnabled[p.key]} />
          {#if paramEnabled[p.key]}
            <TableBaseRow>
              <LabeledSlider
                bind:value={paramValues[p.key]}
                min={p.min}
                max={p.max}
                step={p.step}
                showValue={true}
              />
            </TableBaseRow>
          {/if}
        {/each}
      </TableGroup>
    </div>
  </div>
</div>

<!-- Skill selection modal: search + two-column card grid; the whole card toggles selection. -->
<Modal bind:open={skillsModalOpen} title={t("agent.form.skillsTitle")}>
  <div class="flex h-[65vh] w-[680px] max-w-[85vw] flex-col pt-14">
    <div class="border-b border-[var(--hairline)] px-5 pb-4">
      <div class="relative">
        <Search
          class="absolute left-3 top-1/2 -translate-y-1/2 text-base-content/40"
          size={15}
        />
        <input
          type="text"
          bind:value={skillSearch}
          placeholder={t("agent.form.searchSkills")}
          class="w-full rounded-lg border border-[var(--hairline)] bg-base-200 py-2 pl-9 pr-3 text-sm outline-none placeholder:text-base-content/35 focus:border-[var(--field-border-hover)]"
        />
      </div>
    </div>

    <div class="flex-1 overflow-y-auto p-5">
      {#if filteredSkills.length === 0 && missingSelectedSkills.length === 0}
        <div class="flex h-full items-center justify-center">
          <p class="text-sm text-base-content/45">{t("agent.form.noSkills")}</p>
        </div>
      {:else}
        <div class="grid grid-cols-2 gap-3">
          {#each filteredSkills as skill (skill.name)}
            {@const selected = isSkillSelected(skill.name)}
            <button
              type="button"
              aria-pressed={selected}
              class="flex flex-col rounded-xl border p-4 text-left transition-colors {selected
                ? 'border-primary/50 bg-primary/5'
                : 'border-[var(--hairline)] bg-[var(--bg-panel)] hover:border-[var(--hairline-strong)]'}"
              onclick={() => toggleSkill(skill.name, !selected)}
            >
              <div class="flex w-full items-center justify-between gap-2">
                <span class="truncate text-sm font-medium text-base-content">
                  {skill.name}
                </span>
                {#if selected}
                  <Check size={15} class="shrink-0 text-primary" />
                {/if}
              </div>
              {#if skill.disabled}
                <span class="mt-0.5 text-xs text-base-content/40">
                  {t("agent.form.skillDisabled")}
                </span>
              {/if}
              {#if skill.description}
                <p
                  class="mt-1.5 line-clamp-2 text-xs leading-relaxed text-base-content/50"
                >
                  {skill.description}
                </p>
              {/if}
            </button>
          {/each}
          <!-- Linked skills missing on disk: dimmed card, click to unlink. -->
          {#each missingSelectedSkills as name (name)}
            <button
              type="button"
              class="flex flex-col rounded-xl border border-primary/30 bg-[var(--bg-panel)] p-4 text-left opacity-60"
              onclick={() => toggleSkill(name, false)}
            >
              <div class="flex w-full items-center justify-between gap-2">
                <span class="truncate text-sm font-medium text-base-content/60">
                  {name}
                </span>
                <Check size={15} class="shrink-0 text-primary/60" />
              </div>
              <span class="mt-0.5 text-xs text-base-content/40">
                {t("agent.form.skillMissing")}
              </span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</Modal>

<!-- MCP server selection modal: the whole card toggles; execution mode is configured inside selected cards. -->
<Modal bind:open={mcpModalOpen} title={t("agent.form.mcpServers")}>
  <div class="flex max-h-[65vh] w-[680px] max-w-[85vw] flex-col pt-14">
    <div class="flex-1 overflow-y-auto p-5 pt-1">
      {#if availableServers.length === 0}
        <div class="px-3 py-10 text-center">
          <p class="text-sm text-base-content/55">
            {t("agent.input.noAvailableMcpServers")}
          </p>
          <p class="mt-1 text-xs text-base-content/40">
            {t("agent.input.configureMcpInSettings")}
          </p>
        </div>
      {:else}
        <div class="grid grid-cols-2 gap-3">
          {#each availableServers as server (server.id)}
            {@const selected = isMcpSelected(server.id)}
            <!-- div with role="button" instead of <button>: selected cards embed
                 a real Select button and HTML forbids nested buttons. -->
            <div
              role="button"
              tabindex="0"
              aria-pressed={selected}
              class="flex cursor-default flex-col rounded-xl border p-4 text-left transition-colors {selected
                ? 'border-primary/50 bg-primary/5'
                : 'border-[var(--hairline)] bg-[var(--bg-panel)] hover:border-[var(--hairline-strong)]'}"
              onclick={() => toggleMcp(server.id, !selected)}
              onkeydown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  toggleMcp(server.id, !selected);
                }
              }}
            >
              <div class="flex w-full items-center justify-between gap-2">
                <span class="truncate text-sm font-medium text-base-content">
                  {server.displayName ?? server.name}
                </span>
                {#if selected}
                  <Check size={15} class="shrink-0 text-primary" />
                {/if}
              </div>
              <span class="mt-1.5 text-xs text-base-content/50">
                {t("agent.input.enabledToolsCount", {
                  count: server.enabledTools.length,
                })}
              </span>
              {#if selected}
                <!-- Wrapper stops propagation so configuring the mode doesn't toggle the card. -->
                <div
                  class="mt-2.5 flex items-center justify-between gap-2"
                  role="none"
                  onclick={(e) => e.stopPropagation()}
                  onkeydown={(e) => e.stopPropagation()}
                >
                  <span class="text-xs text-base-content/45">
                    {t("agent.form.toolExecution")}
                  </span>
                  <Select
                    options={executionModeOptions}
                    selectedValue={mcpMode(server.id)}
                    onSelect={(value) =>
                      setMcpMode(server.id, value as "auto" | "manual")}
                    size="sm"
                    autoWidth={true}
                  />
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</Modal>

<!-- Close the icon picker on outside click; clicks inside .icon-picker keep it open. -->
<svelte:window onclick={handleIconPickerOutside} />
