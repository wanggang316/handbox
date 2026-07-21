<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { ArrowLeft, ChevronDown, ChevronRight, Save } from "@lucide/svelte";
  import Button from "../ui/Button.svelte";
  import Select from "../ui/Select.svelte";
  import Toggle from "../ui/Toggle.svelte";
  import LabeledSlider from "../ui/LabeledSlider.svelte";
  import { AGENT_ICONS } from "$lib/utils/agentIcons";
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

  // ── 模型参数：会话/引擎实际消费的采样参数，扁平绘制（label + toggle + slider）。
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

  // 折叠区标题：与 .form-section-label 同观感；按钮需 flex 排布 chevron，
  // 而该类是无层级 CSS（display: block 会压过 .flex），故用等效 utilities。
  const SECTION_TOGGLE_CLASS =
    "flex items-center gap-1.5 text-left text-[11px] font-semibold uppercase tracking-wider text-base-content/45 transition-colors hover:text-base-content/70";

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

  let paramsOpen = $state(false);
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

<!-- Agent 编辑二级页：与 GenUI 编辑页同构的容器（居中 max-w-5xl + 页边距），
     单列纵排（不再左右结构）。 -->
<div class="h-full flex flex-col">
  <!-- 顶部工具栏 -->
  <div class="flex-shrink-0 border-b border-base-300 px-6 pb-4 pt-12">
    <div class="mx-auto w-full max-w-5xl">
      <button
        class="flex items-center gap-2 text-sm text-base-content/70 hover:text-base-content w-fit mb-4"
        onclick={backToList}
      >
        <ArrowLeft size={14} />
        {t("agent.form.backToList")}
      </button>

      <div class="flex items-center gap-3">
        <div class="min-w-0 flex-1">
          <input
            class="modal-title-input w-full"
            bind:value={formData.name}
            placeholder={t("agent.form.namePlaceholder")}
            disabled={isBuiltin}
          />
          <input
            class="mt-1 w-full bg-transparent text-sm text-base-content/80 outline-none placeholder:text-base-content/35"
            bind:value={formData.description}
            placeholder={t("agent.form.descriptionPlaceholder")}
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

  <!-- 表单主体：单列纵排各配置区 -->
  <div class="flex-1 min-h-0 overflow-y-auto px-6 py-5">
    <div class="mx-auto flex w-full max-w-5xl flex-col gap-6">
      <!-- 图标：精选 Lucide 图标网格；再次点选中项可清除（回退默认图标） -->
      <div class="flex flex-col gap-2">
        <span class="form-section-label">{t("agent.form.iconLabel")}</span>
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
                : 'border-[var(--hairline)] text-base-content/55 hover:border-[var(--hairline-strong)] hover:text-base-content'}"
              onclick={() =>
                (formData.icon = formData.icon === opt.name ? "" : opt.name)}
            >
              <Icon size={16} />
            </button>
          {/each}
        </div>
      </div>

      <!-- 系统提示词 -->
      <div class="flex flex-col gap-2.5 border-t border-[var(--hairline)] pt-5">
        <div class="flex items-baseline justify-between">
          <span class="form-section-label">{t("agent.form.systemPromptTitle")}</span>
          <span class="text-xs text-base-content/35">
            {t("agent.form.charCount", { count: formData.systemPrompt.length })}
          </span>
        </div>
        <textarea
          class="field min-h-64 w-full resize-y px-3 py-2.5 font-mono text-sm leading-relaxed"
          bind:value={formData.systemPrompt}
          placeholder={t("agent.systemPrompt.placeholder")}
        ></textarea>
      </div>

      <!-- 工具：内置工具 / 执行方式 / 技能 / MCP 服务器——所有工具面配置归一组 -->
      <div class="flex flex-col gap-3.5 border-t border-[var(--hairline)] pt-5">
        <span class="form-section-label">{t("agent.form.sectionTools")}</span>

        <div class="flex flex-col gap-1.5">
          <span class="text-xs text-base-content/70">
            {t("agent.form.builtinTools")}
          </span>
          <!-- chip 式多选（选中高亮） -->
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
        </div>

        <div class="flex flex-col gap-1">
          <span class="text-xs text-base-content/70">
            {t("agent.form.toolExecution")}
          </span>
          <Select
            options={toolExecutionModeOptions}
            bind:selectedValue={formData.toolExecutionMode}
            size="sm"
          />
        </div>

        <!-- 关联 skill：与 MCP 同构的「定义携带、运行消费」机制——勾选的 skill
             对该 Agent 的所有会话每轮固定注入（全局禁用优先）。 -->
        <div class="flex flex-col gap-1.5">
          <span class="text-xs text-base-content/70">
            {t("agent.form.skillsTitle")}
          </span>
          {#if availableSkills.length === 0 && missingSelectedSkills.length === 0}
            <div
              class="rounded-md border border-dashed border-[var(--hairline)] px-3 py-3 text-center"
            >
              <p class="text-xs text-base-content/55">
                {t("agent.form.noSkills")}
              </p>
            </div>
          {:else}
            <div class="flex flex-col gap-2">
              {#each availableSkills as skill (skill.name)}
                <div class="flex items-center justify-between gap-2">
                  <div class="min-w-0 flex-1">
                    <span class="block truncate text-sm text-base-content/85">
                      {skill.name}
                      {#if skill.disabled}
                        <span class="ml-1 text-xs text-base-content/40">
                          {t("agent.form.skillDisabled")}
                        </span>
                      {/if}
                    </span>
                    {#if skill.description}
                      <span class="block truncate text-xs text-base-content/40">
                        {skill.description}
                      </span>
                    {/if}
                  </div>
                  <Toggle
                    checked={isSkillSelected(skill.name)}
                    onChange={(v) => toggleSkill(skill.name, v)}
                  />
                </div>
              {/each}
              <!-- 已关联但已不存在的 skill（被删 / 改名）：保留成可取消的行 -->
              {#each missingSelectedSkills as name (name)}
                <div class="flex items-center justify-between gap-2">
                  <span class="min-w-0 flex-1 truncate text-sm text-base-content/40">
                    {name}
                    <span class="ml-1 text-xs">{t("agent.form.skillMissing")}</span>
                  </span>
                  <Toggle checked={true} onChange={() => toggleSkill(name, false)} />
                </div>
              {/each}
            </div>
          {/if}
        </div>

        <div class="flex flex-col gap-1.5">
          <span class="text-xs text-base-content/70">
            {t("agent.form.mcpServers")}
          </span>
          {#if availableServers.length === 0}
            <div
              class="rounded-md border border-dashed border-[var(--hairline)] px-3 py-3 text-center"
            >
              <p class="text-xs text-base-content/55">
                {t("agent.input.noAvailableMcpServers")}
              </p>
              <p class="mt-0.5 text-xs text-base-content/40">
                {t("agent.input.configureMcpInSettings")}
              </p>
            </div>
          {:else}
            <div class="flex flex-col gap-3">
              {#each availableServers as server (server.id)}
                <div class="flex flex-col gap-1">
                  <div class="flex items-center justify-between gap-2">
                    <span class="truncate text-sm text-base-content/85">
                      {server.displayName ?? server.name}
                    </span>
                    <Toggle
                      checked={isMcpSelected(server.id)}
                      onChange={(v) => toggleMcp(server.id, v)}
                    />
                  </div>
                  <div class="flex items-center justify-between gap-2">
                    <span class="text-xs text-base-content/40">
                      {t("agent.input.enabledToolsCount", {
                        count: server.enabledTools.length,
                      })}
                    </span>
                    {#if isMcpSelected(server.id)}
                      <Select
                        options={executionModeOptions}
                        selectedValue={mcpMode(server.id)}
                        onSelect={(value) =>
                          setMcpMode(server.id, value as "auto" | "manual")}
                        size="sm"
                        autoWidth={true}
                      />
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>

      <!-- 工作目录：运行环境配置，独立于工具组 -->
      <div class="flex flex-col gap-2.5 border-t border-[var(--hairline)] pt-5">
        <span class="form-section-label">{t("agent.form.workingDir")}</span>
        <Select
          options={workingDirModeOptions}
          bind:selectedValue={formData.workingDirMode}
          size="sm"
        />
      </div>

      <!-- 生成式 UI -->
      <div class="flex flex-col gap-2.5 border-t border-[var(--hairline)] pt-5">
        <div class="flex items-center justify-between gap-3">
          <div class="flex min-w-0 flex-col gap-0.5">
            <span class="text-sm text-base-content/85">
              {t("agent.form.generativeUi")}
            </span>
            <span class="text-xs text-base-content/50">
              {t("agent.form.generativeUiDesc")}
            </span>
          </div>
          <Toggle bind:checked={formData.generativeUi} />
        </div>
        {#if formData.generativeUi}
          <div class="flex flex-col gap-1">
            <Select
              options={genuiOptions}
              bind:selectedValue={formData.genuiId}
              size="sm"
            />
            <span class="text-xs text-base-content/50">
              {t("agent.form.genuiHint")}
            </span>
          </div>
        {/if}
      </div>

      <!-- 模型参数（折叠） -->
      <div class="flex flex-col gap-3 border-t border-[var(--hairline)] pt-5 pb-6">
        <button
          type="button"
          class={SECTION_TOGGLE_CLASS}
          onclick={() => (paramsOpen = !paramsOpen)}
        >
          {#if paramsOpen}
            <ChevronDown size={13} />
          {:else}
            <ChevronRight size={13} />
          {/if}
          {t("agent.form.modelParams")}
        </button>
        {#if paramsOpen}
          <div class="flex max-w-md flex-col gap-3">
            {#each PARAM_META as p (p.key)}
              <div class="flex flex-col gap-2">
                <div class="flex items-center justify-between">
                  <span class="text-sm text-base-content/85">{p.label}</span>
                  <Toggle bind:checked={paramEnabled[p.key]} />
                </div>
                {#if paramEnabled[p.key]}
                  <LabeledSlider
                    bind:value={paramValues[p.key]}
                    min={p.min}
                    max={p.max}
                    step={p.step}
                    showValue={true}
                  />
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>
