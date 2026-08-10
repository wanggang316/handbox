<script lang="ts">
  import { onMount } from "svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Spinner from "$lib/components/ui/Spinner.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import Input from "$lib/components/ui/Input.svelte";
  import Textarea from "$lib/components/ui/Textarea.svelte";
  import InfoTooltip from "$lib/components/ui/InfoTooltip.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import FormModal from "$lib/components/ui/FormModal.svelte";
  import ConfirmModal from "$lib/components/ui/ConfirmModal.svelte";
  import {
    listHookRules,
    createHookRule,
    updateHookRule,
    deleteHookRule,
  } from "$lib/api";
  import type {
    CreateHookRuleRequest,
    HookAction,
    HookEvent,
    HookRule,
  } from "$lib/types";
  import { t } from "$lib/i18n";
  import { Plus, Trash2, Pencil } from "@lucide/svelte";

  let rules = $state<HookRule[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let editorOpen = $state(false);
  let saving = $state(false);
  let saveError = $state<string | null>(null);
  /** The rule being edited; null means the editor is creating a new one. */
  let editing = $state<HookRule | null>(null);

  let deleteTarget = $state<HookRule | null>(null);

  // Form fields, kept flat so the editor works the same for create and edit.
  let formName = $state("");
  let formEvent = $state<HookEvent>("before_tool_call");
  let formToolPattern = $state("*");
  let formArgField = $state("");
  let formArgContains = $state("");
  let formAction = $state<HookAction>("run_command");
  let formMessage = $state("");
  let formCommand = $state("");

  // A prompt or a finished turn has no tool name to glob and no arguments to
  // inspect; showing those fields would invite rules that silently never match.
  const matchesTool = $derived(
    formEvent !== "user_prompt_submit" && formEvent !== "turn_end",
  );

  // The command field only means something for run_command, and leaving a
  // stale command visible under another action reads as if it would still run.
  const needsCommand = $derived(formAction === "run_command");

  const eventOptions = $derived([
    { value: "before_tool_call", label: t("settings.hooks.event.before") },
    { value: "after_tool_call", label: t("settings.hooks.event.after") },
    { value: "user_prompt_submit", label: t("settings.hooks.event.prompt") },
    { value: "turn_end", label: t("settings.hooks.event.turnEnd") },
    { value: "approval_requested", label: t("settings.hooks.event.approval") },
  ]);

  const actionOptions = $derived(
    (["run_command", "notify"] as HookAction[]).map((action) => ({
      value: action,
      label: actionLabel(action),
    })),
  );

  function actionLabel(action: HookAction): string {
    switch (action) {
      case "notify":
        return t("settings.hooks.action.notify");
      case "run_command":
        return t("settings.hooks.action.runCommand");
      default:
        return action;
    }
  }

  function eventLabel(event: HookEvent): string {
    switch (event) {
      case "before_tool_call":
        return t("settings.hooks.event.before");
      case "after_tool_call":
        return t("settings.hooks.event.after");
      case "user_prompt_submit":
        return t("settings.hooks.event.prompt");
      case "turn_end":
        return t("settings.hooks.event.turnEnd");
      case "approval_requested":
        return t("settings.hooks.event.approval");
      default:
        return event;
    }
  }

  /** Human-readable summary of what a rule matches, for the list row. */
  function conditionSummary(rule: HookRule): string {
    if (rule.event === "user_prompt_submit" || rule.event === "turn_end") {
      const subject =
        rule.event === "user_prompt_submit"
          ? t("settings.hooks.promptSubject")
          : t("settings.hooks.replySubject");
      return rule.argContains ? `${subject} ⊃ "${rule.argContains}"` : subject;
    }
    if (!rule.argContains) {
      return rule.toolPattern;
    }
    const field = rule.argField ?? t("settings.hooks.anyArgument");
    return `${rule.toolPattern} · ${field} ⊃ "${rule.argContains}"`;
  }

  onMount(load);

  async function load() {
    loading = true;
    error = null;
    try {
      rules = await listHookRules();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function openCreate() {
    editing = null;
    formName = "";
    formEvent = "before_tool_call";
    formToolPattern = "*";
    formArgField = "";
    formArgContains = "";
    formAction = "run_command";
    formMessage = "";
    formCommand = "";
    saveError = null;
    editorOpen = true;
  }

  function openEdit(rule: HookRule) {
    editing = rule;
    formName = rule.name;
    formEvent = rule.event;
    formToolPattern = rule.toolPattern;
    formArgField = rule.argField ?? "";
    formArgContains = rule.argContains ?? "";
    formAction = rule.action;
    formMessage = rule.message ?? "";
    formCommand = rule.command ?? "";
    saveError = null;
    editorOpen = true;
  }

  async function handleSave() {
    if (!formName.trim()) return;
    if (matchesTool && !formToolPattern.trim()) return;
    if (needsCommand && !formCommand.trim()) return;
    saving = true;
    saveError = null;
    try {
      if (editing) {
        // Empty strings clear the nullable fields, which is exactly what an
        // emptied input means here.
        await updateHookRule(editing.id, {
          name: formName.trim(),
          event: formEvent,
          toolPattern: formToolPattern.trim(),
          argField: formArgField.trim(),
          argContains: formArgContains.trim(),
          action: formAction,
          message: formMessage.trim(),
          command: formCommand.trim(),
        });
      } else {
        const request: CreateHookRuleRequest = {
          name: formName.trim(),
          event: formEvent,
          toolPattern: formToolPattern.trim(),
          argField: formArgField.trim() || null,
          argContains: formArgContains.trim() || null,
          action: formAction,
          message: formMessage.trim() || null,
          command: formCommand.trim() || null,
        };
        await createHookRule(request);
      }
      editorOpen = false;
      await load();
    } catch (e) {
      saveError = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
  }

  // Non-optimistic: on failure the reload restores the stored state, so the
  // toggle never shows a value the backend did not accept.
  async function handleToggle(rule: HookRule, enabled: boolean): Promise<boolean> {
    try {
      await updateHookRule(rule.id, { enabled });
      await load();
      return true;
    } catch (e) {
      console.error("Failed to toggle hook rule:", e);
      return false;
    }
  }

  async function handleDelete() {
    const target = deleteTarget;
    if (!target) return;
    try {
      await deleteHookRule(target.id);
      await load();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      deleteTarget = null;
    }
  }
</script>

<div class="p-6 pr-8 pt-2 flex flex-col gap-y-4">
  <div class="flex items-start justify-between gap-4">
    <p class="text-xs text-base-content/60 mt-0.5">
      {t("settings.hooks.description")}
    </p>
    <Button variant="gray" size="sm" onclick={openCreate}>
      <Plus size={14} />
      {t("settings.hooks.add")}
    </Button>
  </div>

  {#if loading && rules.length === 0}
    <div class="flex items-center justify-center py-10">
      <Spinner size={28} />
    </div>
  {/if}

  {#if error}
    <div class="rounded-lg bg-error/10 px-4 py-3 text-sm text-error">
      {error}
    </div>
  {/if}

  {#if !loading && rules.length === 0}
    <div class="rounded-xl border border-[var(--hairline)] px-6 py-10 text-center">
      <p class="text-sm text-base-content/70">{t("settings.hooks.empty")}</p>
      <p class="mt-1 text-xs text-base-content/50">
        {t("settings.hooks.emptyHint")}
      </p>
    </div>
  {:else if rules.length > 0}
    <div class="rounded-xl overflow-hidden">
      <TableGroup>
        {#each rules as rule (rule.id)}
          <div class="w-full px-6 py-4">
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0">
                <div class="flex items-center gap-2">
                  <span class="text-sm font-medium truncate">{rule.name}</span>
                  <span
                    class="shrink-0 rounded px-1.5 py-0.5 text-[10px] bg-base-content/10 text-base-content/70"
                  >
                    {eventLabel(rule.event)} · {actionLabel(rule.action)}
                  </span>
                </div>
                <p class="mt-1 text-xs text-base-content/60 font-mono truncate">
                  {conditionSummary(rule)}
                </p>
                {#if rule.command}
                  <p class="mt-1 text-xs text-base-content/50 font-mono truncate">
                    $ {rule.command}
                  </p>
                {/if}
                {#if rule.message}
                  <p class="mt-1 text-xs text-base-content/50 truncate">
                    {rule.message}
                  </p>
                {/if}
              </div>

              <div class="flex items-center gap-2 shrink-0">
                <Toggle
                  checked={rule.enabled}
                  onChangeBefore={(enabled) => handleToggle(rule, enabled)}
                />
                <Button variant="ghost" size="sm" onclick={() => openEdit(rule)}>
                  <Pencil size={14} />
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onclick={() => (deleteTarget = rule)}
                >
                  <Trash2 size={14} />
                </Button>
              </div>
            </div>
          </div>
        {/each}
      </TableGroup>
    </div>
  {/if}
</div>

<FormModal
  bind:open={editorOpen}
  title={editing ? t("settings.hooks.editTitle") : t("settings.hooks.addTitle")}
  {saving}
  error={saveError}
  submitLabel={t("common.save")}
  submitDisabled={!formName.trim() || !formToolPattern.trim()}
  onSubmit={handleSave}
  onClose={() => (editorOpen = false)}
>
  <div class="flex flex-col gap-4">
    <Input
      label={t("settings.hooks.field.name")}
      bind:value={formName}
      placeholder={t("settings.hooks.field.namePlaceholder")}
    />

    <div class="grid grid-cols-2 gap-4">
      <Select
        label={t("settings.hooks.field.event")}
        bind:value={formEvent}
        options={eventOptions}
      />
      <Select
        label={t("settings.hooks.field.action")}
        bind:value={formAction}
        options={actionOptions}
      />
    </div>

    {#if matchesTool}
      <div>
        {@render labelWithHelp(
          t("settings.hooks.field.toolPattern"),
          t("settings.hooks.field.hint"),
          "w-64",
        )}
        <Input bind:value={formToolPattern} placeholder="write" literal />
      </div>

      <!-- Input renders its label and control as siblings, so each needs a
           wrapper to occupy a single grid cell — bare, they split into four. -->
      <div class="grid grid-cols-2 gap-4">
        <div>
          <Input
            label={t("settings.hooks.field.argField")}
            bind:value={formArgField}
            placeholder="path"
            literal
          />
        </div>
        <div>
          <Input
            label={t("settings.hooks.field.argContains")}
            bind:value={formArgContains}
            placeholder=".md"
            literal
          />
        </div>
      </div>
    {:else if formEvent === "user_prompt_submit"}
      <Input
        label={t("settings.hooks.field.promptContains")}
        bind:value={formArgContains}
        placeholder={t("settings.hooks.field.promptContainsPlaceholder")}
        literal
      />
    {:else}
      <Input
        label={t("settings.hooks.field.replyContains")}
        bind:value={formArgContains}
        placeholder={t("settings.hooks.field.replyContainsPlaceholder")}
        literal
      />
    {/if}

    {#if needsCommand}
      <div>
        {@render labelWithHelp(
          t("settings.hooks.field.command"),
          t("settings.hooks.field.commandHint"),
          "w-80",
        )}
        <Textarea
          bind:value={formCommand}
          rows={3}
          placeholder={"npx prettier --write ."}
          literal
        />
      </div>
    {/if}

    <Input
      label={t("settings.hooks.field.message")}
      bind:value={formMessage}
      placeholder={t("settings.hooks.field.messagePlaceholder")}
    />
  </div>
</FormModal>

{#snippet labelWithHelp(label: string, help: string, helpWidth: string)}
  <div class="mb-2 flex items-center gap-1.5">
    <span class="text-sm font-medium">{label}</span>
    <InfoTooltip content={help} width={helpWidth} />
  </div>
{/snippet}

<ConfirmModal
  open={deleteTarget !== null}
  title={t("settings.hooks.deleteTitle")}
  message={deleteTarget
    ? t("settings.hooks.deleteMessage").replace("{name}", deleteTarget.name)
    : ""}
  confirmText={t("common.delete")}
  onConfirm={handleDelete}
  onCancel={() => (deleteTarget = null)}
/>
