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

  interface Props {
    // 编辑模式传入既有 Agent；新建模式留空
    agent?: Agent | null;
  }

  interface AgentFormData {
    name: string;
    // Lucide kebab-case 图标名；空串表示用默认图标
    icon: string;
    temperature?: number;
    maxTokens?: number;
    systemPrompt: string;
    // 关联的 skill 名单（按名引用已发现的 skill；运行时每轮固定注入）
    skills: string[];
    mcpServers: McpServerConfig[];
    generativeUi: boolean;
    // 关联的 GenUI id；空串表示未关联
    genuiId: string;
    description: string;
    builtinTools: string[];
    workingDirMode: string;
    toolExecutionMode: string;
  }

  let { agent = null }: Props = $props();

  // ── 模型参数：会话/引擎实际消费的采样参数（label + toggle + slider）。
  //    仅 temperature / maxTokens —— top_p / top_k 会话层与引擎均不消费，故不在此暴露。 ──
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

  // ── 能力（Capability）：内置工具 / 工作目录 / 工具执行 ──
  // coding-agent 内置工具名（与后端 builtinTools 取值对齐）。
  const BUILTIN_TOOLS = ["read", "write", "edit", "bash", "grep", "find", "ls"];
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

  // 内置 Agent：名称只读、不可删除（由后端约束）。
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

  // 可关联的 skill 列表：定义级关联与具体项目无关，不传 workingDir（只发现
  // user / appData 两档）；仅列校验通过的干净 skill。已关联但磁盘上已消失的
  // 名字仍显示为附加行（可取消关联），运行时未知名静默跳过、不会报错。
  let availableSkills = $state<SkillInfo[]>([]);

  // Skill / MCP 的选择在 Modal 弹窗中进行（行上仅显示已关联数量）。
  let skillsModalOpen = $state(false);
  let mcpModalOpen = $state(false);

  // 图标选择浮层：点标题前的图标按钮原地弹出；选择即替换并关闭，
  // 点击当前选中项清除（回退默认 Bot）。点击浮层外关闭。
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

  // 技能 Modal 内搜索（按名称 / 描述过滤，大小写不敏感）。
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

  // 当前图标（未设置 / 未识别回退默认 Bot）。
  const CurrentIcon = $derived(resolveAgentIcon(formData.icon));

  // 已关联但不在发现列表里的名字（skill 被删 / 改名）：保留成可取消的行。
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

  /** 把表单落库：编辑 = 逐字段比较下发；新建 = create 后对非默认能力字段补写。 */
  async function persist(data: AgentFormData) {
    // 关联的 GenUI 仅在开启生成式 UI 时有效；关闭时清空关联。
    const effectiveGenuiId =
      data.generativeUi && data.genuiId ? data.genuiId : null;

    if (agent?.id) {
      // 更新现有 Agent。仅在名称实际变化时才写：后端拒绝重命名内置 Agent
      // （"Builtin agent cannot be renamed"），无条件下发会让「只改图标等其它
      // 字段」的内置 Agent 编辑在第一步就失败。
      if (data.name !== agent.name) {
        await agentActions.updateAgentName(agent.id, data.name);
      }

      // 图标：空串归一为 null（清除自定义图标，回退默认）。
      if ((data.icon || null) !== (agent.icon ?? null)) {
        await agentActions.updateAgentField(agent.id, "icon", data.icon || null);
      }

      // Helper function to compare optional values
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

      // MCP 服务器变更（序列化比较，避免无意义写入）
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

      // 生成式 UI: 显式比较布尔值，关闭时必须发送 false（不能被假值跳过）
      if ((data.generativeUi ?? false) !== (agent.generativeUi ?? false)) {
        await agentActions.updateAgentField(
          agent.id,
          "generativeUi",
          data.generativeUi ?? false
        );
      }

      // 关联 GenUI: 与既有值比较，变更时下发（null 表示解除关联）
      if ((agent.genuiId ?? null) !== effectiveGenuiId) {
        await agentActions.updateAgentField(agent.id, "genuiId", effectiveGenuiId);
      }

      // 关联 skill 变更（序列化比较，避免无意义写入）
      if (
        JSON.stringify(data.skills ?? []) !== JSON.stringify(agent.skills ?? [])
      ) {
        await agentActions.updateAgentField(agent.id, "skills", data.skills);
      }

      // 能力字段：后端仅支持逐字段更新，变更时下发。
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
      // 创建新 Agent（后端 create 不接受能力字段，需创建后逐项写入）
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

      // 仅对非默认能力字段做 create-then-update。
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

<!-- Agent 编辑二级页：设置子页的样式语言——居中 max-w-3xl 阅读宽度（不撑满屏幕）、
     TableGroup 分组卡 + 行组件；Skill / MCP 的选择经 Modal 弹窗。 -->
<div class="h-full flex flex-col">
  <!-- 顶部工具栏 -->
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
        <!-- 图标：标题前的当前图标按钮（默认 Bot），点击原地弹出选择浮层 -->
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

  <!-- 表单主体：设置页式分组卡纵排 -->
  <div class="flex-1 min-h-0 overflow-y-auto px-6 pb-6">
    <div class="mx-auto flex w-full max-w-3xl flex-col gap-y-4">

      <!-- 系统提示词 -->
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

      <!-- 工具：内置工具 / 执行方式 / 技能 / MCP（技能与 MCP 经 Modal 选择） -->
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

      <!-- 运行：工作目录 / 生成式 UI -->
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

      <!-- 模型参数（可折叠分组卡） -->
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

<!-- 技能选择 Modal：搜索 + 双列卡片网格，整卡点击切换选中（Directory 式）。 -->
<Modal bind:open={skillsModalOpen} title={t("agent.form.skillsTitle")}>
  <div class="flex h-[65vh] w-[680px] max-w-[85vw] flex-col pt-14">
    <!-- 搜索框 -->
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
          class="w-full rounded-lg border border-[var(--hairline)] bg-base-200 py-2 pl-9 pr-3 text-sm outline-none placeholder:text-base-content/35 focus:border-primary"
        />
      </div>
    </div>

    <!-- 卡片网格（滚动区） -->
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
          <!-- 已关联但已不存在的 skill（被删 / 改名）：暗淡卡，点击取消关联 -->
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

<!-- MCP 服务器选择 Modal：双列卡片网格，整卡点击切换；选中卡内配置执行方式。 -->
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
            <!-- 宿主是 role="button" 的 div 而非 <button>：选中态卡内嵌执行方式
                 Select（真按钮），HTML 禁止 button 嵌套。 -->
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
                <!-- 执行方式：选中后卡内配置；包一层拦截点击避免误触整卡切换 -->
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

<!-- 点击浮层外关闭图标选择（浮层与触发按钮在 .icon-picker 内，点击其内不关闭） -->
<svelte:window onclick={handleIconPickerOutside} />
