<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import { skillState, skillActions } from "$lib/states/skill.svelte";
  import type { SkillInfo, SkillScope } from "$lib/types";
  import {
    LoaderCircle,
    Sparkles,
    RefreshCw,
    FolderOpen,
    ChevronsUpDown,
    AlertTriangle,
  } from "@lucide/svelte";

  let expandedBodies = $state<Record<string, boolean>>({});
  // SvelteSet: plain Set in $state is not deeply reactive, so has() in the
  // Toggle disabled binding would never re-run on add/delete
  let inFlightSkills = new SvelteSet<string>();

  onMount(() => {
    if (!skillState.initialized) {
      skillActions.loadSkills().catch((error) => {
        console.error("Failed to load skills:", error);
      });
    }
  });

  function skillKey(skill: SkillInfo): string {
    return `${skill.scope}:${skill.path}`;
  }

  function toggleBody(skill: SkillInfo) {
    const key = skillKey(skill);
    expandedBodies[key] = !expandedBodies[key];
  }

  function getScopeLabel(scope: SkillScope): string {
    switch (scope) {
      case "user":
        return "用户";
      case "project":
        return "项目";
      case "appData":
        return "应用";
      default:
        return scope;
    }
  }

  async function handleRefresh() {
    if (skillState.isLoading) return;
    try {
      await skillActions.loadSkills(true);
    } catch (error) {
      console.error("Failed to refresh skills:", error);
    }
  }

  async function handleOpenDir(skill: SkillInfo) {
    try {
      const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
      await revealItemInDir(skill.path);
    } catch (error) {
      console.error("[Skills] Failed to reveal skill directory", error);
    }
  }

  // 非乐观提交：把 IPC 写放进 onChangeBefore，返回 false 时 Toggle 自动把
  // 可见态回退到点击前（成功路径上 store 改写后 checked={!skill.disabled}
  // 已与新值一致；失败路径上 store 未被改写，无 prop 变化信号，必须靠返回
  // false 触发 Toggle 自身的 target.checked / checked 回退）。
  async function handleToggleSkillBefore(
    skill: SkillInfo,
    enabled: boolean
  ): Promise<boolean> {
    // Double-click guard: reject overlapping toggles for the same skill
    if (inFlightSkills.has(skill.name)) {
      return false;
    }

    inFlightSkills.add(skill.name);
    try {
      await skillActions.toggleSkill(skill.name, !enabled);
      return true;
    } catch (error) {
      console.error("Failed to toggle skill:", error);
      return false;
    } finally {
      inFlightSkills.delete(skill.name);
    }
  }
</script>

<div class="p-6 pr-8 pt-14 flex flex-col gap-y-4">
  <!-- 头部：标题 + 刷新 -->
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-base font-medium text-base-content">技能</h1>
      <p class="text-xs text-base-content/60 mt-0.5">
        将 SKILL.md 放入技能目录后会在此处展示，可启停有效的技能
      </p>
    </div>
    <Button
      variant="gray"
      size="sm"
      disabled={skillState.isLoading}
      onclick={handleRefresh}
    >
      {#if skillState.isLoading}
        <LoaderCircle size={14} class="animate-spin" />
      {:else}
        <RefreshCw size={14} />
      {/if}
      刷新
    </Button>
  </div>

  <!-- 加载状态 -->
  {#if skillState.isLoading && skillState.skills.length === 0}
    <div class="flex items-center justify-center py-8">
      <LoaderCircle class="h-6 w-6 animate-spin text-base-content/60" />
      <span class="ml-2 text-sm text-base-content/70">正在加载技能...</span>
    </div>
  {/if}

  <!-- 加载错误 -->
  {#if skillState.error}
    <div class="rounded-lg bg-error/10 px-4 py-3 text-sm text-error">
      {skillState.error}
    </div>
  {/if}

  <div class="rounded-xl overflow-hidden">
    <TableGroup>
      {#each skillState.skills as skill (skillKey(skill))}
        {@const hasError = skill.diagnostics.length > 0}
        {@const expanded = expandedBodies[skillKey(skill)]}
        <div class="w-full px-6 py-4">
          <div class="flex items-start justify-between gap-3 mb-1">
            <div class="flex flex-1 min-w-0 flex-col gap-1">
              <div class="flex items-center gap-2 flex-wrap">
                <span class="text-sm font-medium text-base-content break-all">
                  {skill.name}
                </span>
                <span
                  class="px-2 py-0.5 text-xs rounded-full shrink-0 {hasError
                    ? 'bg-error/10 text-error'
                    : 'bg-primary/10 text-primary'}"
                >
                  {getScopeLabel(skill.scope)}
                </span>
              </div>
              {#if skill.description}
                <p class="text-xs text-base-content/70 break-words">
                  {skill.description}
                </p>
              {/if}
            </div>

            <div class="flex items-center gap-2 shrink-0">
              {#if !hasError}
                <Toggle
                  checked={!skill.disabled}
                  disabled={inFlightSkills.has(skill.name)}
                  onChangeBefore={(enabled) =>
                    handleToggleSkillBefore(skill, enabled)}
                />
              {/if}
              <button
                type="button"
                class="flex items-center gap-1 shrink-0 text-xs text-base-content/60 hover:text-base-content hover:bg-base-300 rounded px-2 py-1 transition-colors"
                onclick={() => handleOpenDir(skill)}
              >
                <FolderOpen size={14} />
                <span>打开目录</span>
              </button>
            </div>
          </div>

          <!-- 诊断信息（校验失败项） -->
          {#if hasError}
            <div class="mt-2 flex flex-col gap-1">
              {#each skill.diagnostics as diagnostic}
                <div class="flex items-start gap-1.5 text-xs text-error">
                  <AlertTriangle size={14} class="mt-0.5 shrink-0" />
                  <span class="break-words">{diagnostic}</span>
                </div>
              {/each}
            </div>
          {/if}

          <!-- body 预览展开/收起 -->
          {#if skill.body}
            <div class="mt-2">
              <button
                type="button"
                class="flex items-center gap-1 text-xs text-base-content/60 hover:text-base-content hover:bg-base-300 rounded px-1 -ml-1 py-0.5 transition-colors"
                onclick={() => toggleBody(skill)}
              >
                <span>{expanded ? "收起内容" : "查看内容"}</span>
                <ChevronsUpDown size={12} />
              </button>
              {#if expanded}
                <pre
                  class="mt-2 max-h-80 overflow-auto rounded-lg bg-base-200 p-3 text-xs text-base-content/80 whitespace-pre-wrap break-words font-mono">{skill.body}</pre>
              {/if}
            </div>
          {/if}
        </div>
      {/each}

      <!-- 空状态 -->
      {#if !skillState.isLoading && skillState.skills.length === 0 && !skillState.error}
        <div class="p-8 text-center">
          <Sparkles class="h-12 w-12 text-base-content/50 mx-auto mb-4" />
          <p class="text-base text-base-content/70 mb-1">暂无技能</p>
          <p class="text-sm text-base-content/60">
            在技能目录中放入 SKILL.md 文件，然后点击「刷新」即可在此处看到。
          </p>
        </div>
      {/if}
    </TableGroup>
  </div>
</div>
