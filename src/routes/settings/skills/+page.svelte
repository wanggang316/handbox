<script lang="ts">
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Spinner from "$lib/components/ui/Spinner.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import { skillState, skillActions } from "$lib/states/skill.svelte";
  import type { SkillInfo, SkillScope } from "$lib/types";
  import { t } from "$lib/i18n";
  import {
    LoaderCircle,
    Zap,
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
        return t("settings.skills.scope.user");
      case "project":
        return t("settings.skills.scope.project");
      case "appData":
        return t("settings.skills.scope.appData");
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

  // Non-optimistic commit: the IPC write runs in onChangeBefore, and returning
  // false makes the Toggle revert its visual state. On success the store update
  // already matches checked={!skill.disabled}; on failure the store is untouched
  // so there is no prop-change signal — only the false return triggers the
  // Toggle's own revert.
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

<div class="p-6 pr-8 pt-2 flex flex-col gap-y-4">
  <div class="flex items-center justify-between">
    <div>
      <p class="text-xs text-base-content/60 mt-0.5">
        {t("settings.skills.description")}
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
      {t("common.refresh")}
    </Button>
  </div>

  {#if skillState.isLoading && skillState.skills.length === 0}
    <div class="flex items-center justify-center py-10">
      <Spinner size={28} />
    </div>
  {/if}

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
          <div class="flex items-start justify-between gap-3">
            <div class="flex flex-1 min-w-0 flex-col gap-1">
              <div class="flex items-center gap-2 flex-wrap">
                <span class="text-sm font-medium text-base-content break-all">
                  {skill.name}
                </span>
                <span
                  class="rounded px-1.5 py-0.5 text-[10px] font-medium shrink-0 {hasError
                    ? 'bg-error/10 text-error'
                    : 'bg-base-200 text-base-content/55'}"
                >
                  {getScopeLabel(skill.scope)}
                </span>
              </div>
              {#if skill.description}
                <p class="text-xs text-base-content/70 break-words line-clamp-2">
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
                class="rounded-md p-1.5 text-base-content/45 transition-colors hover:bg-base-content/10 hover:text-base-content"
                title={t("settings.skills.openDir")}
                aria-label={t("settings.skills.openDir")}
                onclick={() => handleOpenDir(skill)}
              >
                <FolderOpen size={15} />
              </button>
            </div>
          </div>

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

          {#if skill.body}
            <div class="mt-2">
              <button
                type="button"
                class="flex items-center gap-1 text-xs text-base-content/60 hover:text-base-content hover:bg-base-300 rounded px-1 -ml-1 py-0.5 transition-colors"
                onclick={() => toggleBody(skill)}
              >
                <span>{expanded ? t("settings.skills.collapseBody") : t("settings.skills.expandBody")}</span>
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

      {#if !skillState.isLoading && skillState.skills.length === 0 && !skillState.error}
        <div class="p-8 text-center">
          <Zap class="h-12 w-12 text-base-content/50 mx-auto mb-4" />
          <p class="text-base text-base-content/70 mb-1">{t("settings.skills.empty")}</p>
          <p class="text-sm text-base-content/60">
            {t("settings.skills.emptyHint")}
          </p>
        </div>
      {/if}
    </TableGroup>
  </div>
</div>
