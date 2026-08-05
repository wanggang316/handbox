<script lang="ts">
  import { onMount } from "svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Spinner from "$lib/components/ui/Spinner.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import Input from "$lib/components/ui/Input.svelte";
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
  let formAction = $state<HookAction>("ask");
  let formMessage = $state("");

  // A decision action only applies before a call and `notify` only after, which
  // the backend enforces; the picker follows so an invalid pair is unreachable.
  const actionsForEvent = $derived<HookAction[]>(
    formEvent === "before_tool_call" ? ["deny", "ask", "allow"] : ["notify"],
  );

  const eventOptions = $derived([
    { value: "before_tool_call", label: t("settings.hooks.event.before") },
    { value: "after_tool_call", label: t("settings.hooks.event.after") },
  ]);

  const actionOptions = $derived(
    actionsForEvent.map((action) => ({
      value: action,
      label: actionLabel(action),
    })),
  );

  function actionLabel(action: HookAction): string {
    switch (action) {
      case "deny":
        return t("settings.hooks.action.deny");
      case "ask":
        return t("settings.hooks.action.ask");
      case "allow":
        return t("settings.hooks.action.allow");
      case "notify":
        return t("settings.hooks.action.notify");
      default:
        return action;
    }
  }

  function eventLabel(event: HookEvent): string {
    return event === "before_tool_call"
      ? t("settings.hooks.event.before")
      : t("settings.hooks.event.after");
  }

  /** Human-readable summary of what a rule matches, for the list row. */
  function conditionSummary(rule: HookRule): string {
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
    formAction = "ask";
    formMessage = "";
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
    saveError = null;
    editorOpen = true;
  }

  // Switching the event can strand an action that is invalid for it; snap to the
  // first legal one rather than letting the backend reject the save.
  $effect(() => {
    if (!actionsForEvent.includes(formAction)) {
      formAction = actionsForEvent[0];
    }
  });

  async function handleSave() {
    if (!formName.trim() || !formToolPattern.trim()) return;
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

    <Input
      label={t("settings.hooks.field.toolPattern")}
      bind:value={formToolPattern}
      placeholder="bash"
    />

    <div class="grid grid-cols-2 gap-4">
      <Input
        label={t("settings.hooks.field.argField")}
        bind:value={formArgField}
        placeholder="command"
      />
      <Input
        label={t("settings.hooks.field.argContains")}
        bind:value={formArgContains}
        placeholder="rm -rf"
      />
    </div>

    <Input
      label={t("settings.hooks.field.message")}
      bind:value={formMessage}
      placeholder={t("settings.hooks.field.messagePlaceholder")}
    />

    <p class="text-xs text-base-content/50">
      {t("settings.hooks.field.hint")}
    </p>
  </div>
</FormModal>

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
