<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { ArrowLeft, Save } from "@lucide/svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import { TableGroup, TableBaseRow } from "$lib/components/ui/table";
  import ScheduleEditor from "$lib/components/jobs/ScheduleEditor.svelte";
  import TargetPicker from "$lib/components/jobs/TargetPicker.svelte";
  import { providerState, providerActions } from "$lib/states/provider.svelte";
  import { agentState, agentActions } from "$lib/states/agent.svelte";
  import { jobStore } from "$lib/stores/jobStore.svelte";
  import { AppError } from "$lib/api";
  import { t } from "$lib/i18n";
  import type { Agent, Job, JobTarget } from "$lib/types";
  import {
    DEFAULT_EXEC_TIMEOUT_SECS,
    DEFAULT_MAX_RETRIES,
    DEFAULT_RETRY_DELAY_SECS,
  } from "$lib/types/job";
  import type { ProviderWithModels } from "$lib/types/provider";

  interface Props {
    /** Existing job in edit mode; null in create mode. */
    job?: Job | null;
  }

  let { job = null }: Props = $props();

  // Form state is a local copy, discarded on leaving the page; the outer job is
  // never mutated.
  const DEFAULT_CRON = "0 9 * * *";
  const localTimezone = Intl.DateTimeFormat().resolvedOptions().timeZone;

  function emptyTarget(): JobTarget {
    return {
      kind: "prompt",
      providerId: "",
      modelId: "",
      prompt: "",
      sessionStrategy: "new_session",
    };
  }

  interface FormState {
    name: string;
    description: string;
    cronExpr: string;
    timezone: string;
    enabled: boolean;
    target: JobTarget;
    // Robustness fields held as strings; empty means "unset" and saves as
    // undefined so the backend default applies.
    execTimeoutSecs: string;
    maxRetries: string;
    retryDelaySecs: string;
  }

  function initialForm(): FormState {
    if (job) {
      return {
        name: job.name,
        description: job.description ?? "",
        cronExpr: job.cronExpr,
        timezone: job.timezone || localTimezone,
        enabled: job.enabled,
        // Deep-copy via $state.snapshot (non-proxy copy) so the outer job's
        // target reference is never mutated.
        target: $state.snapshot(job.target) as JobTarget,
        // Backfill stored values including 0, which means "unlimited / no retry".
        execTimeoutSecs: String(job.execTimeoutSecs),
        maxRetries: String(job.maxRetries),
        retryDelaySecs: String(job.retryDelaySecs),
      };
    }
    return {
      name: "",
      description: "",
      cronExpr: DEFAULT_CRON,
      timezone: localTimezone,
      enabled: true,
      target: emptyTarget(),
      execTimeoutSecs: "",
      maxRetries: "",
      retryDelaySecs: "",
    };
  }

  // The page remounts this component via `{#key job.id}`, so a single init on
  // mount suffices.
  let form = $state<FormState>(initialForm());
  let saving = $state(false);
  let saveError = $state<string | null>(null);
  let showValidation = $state(false);

  // providersWithModels resolves the target's (providerId, modelId) into display
  // names; agents feeds the agent-target dropdown. Both read shared state here
  // so TargetPicker never touches stores directly.
  let providersWithModels = $state<ProviderWithModels[]>([]);
  let agents = $state<Agent[]>([]);
  let agentsLoading = $state(false);

  onMount(() => {
    if (
      providerState.providersWithModelsNeedRefresh ||
      providerState.providersWithModels.length === 0
    ) {
      providerActions.loadProvidersWithModels().catch((e) => {
        console.error("Failed to load providers for job target:", e);
      });
    }

    agentsLoading = true;
    agentActions
      .loadAgents()
      .then(() => {
        agents = agentState.agents;
      })
      .catch((e) => {
        console.error("Failed to load agents for job target:", e);
        agents = [];
      })
      .finally(() => {
        agentsLoading = false;
      });
  });

  // Only enabled providers and their enabled models, matching the chat model picker.
  $effect(() => {
    providersWithModels = providerState.providersWithModels
      .filter((p) => p.enabled)
      .map((p) => ({ ...p, models: p.models.filter((m) => m.enabled) }));
  });

  // Validation mirrors TargetPicker's inline highlighting: prompt targets need
  // provider + model and a non-blank prompt; agent targets need an agent
  // template. Any failure blocks saving.
  const nameError = $derived(
    showValidation && form.name.trim().length === 0
      ? t("jobs.form.nameRequired")
      : null,
  );

  // Empty is valid (saved as undefined, backend default applies); otherwise the
  // value must be a non-negative integer.
  function robustnessError(raw: string, label: string): string | null {
    const trimmed = raw.trim();
    if (trimmed.length === 0) return null; // empty → use default
    const n = Number(trimmed);
    if (!Number.isInteger(n)) return t("jobs.form.mustBeInteger", { label });
    if (n < 0) return t("jobs.form.mustNotBeNegative", { label });
    return null;
  }

  const execTimeoutError = $derived(
    showValidation
      ? robustnessError(form.execTimeoutSecs, t("jobs.form.execTimeoutLabel"))
      : null,
  );
  const maxRetriesError = $derived(
    showValidation
      ? robustnessError(form.maxRetries, t("jobs.form.maxRetriesLabel"))
      : null,
  );
  const retryDelayError = $derived(
    showValidation
      ? robustnessError(form.retryDelaySecs, t("jobs.form.retryDelayLabel"))
      : null,
  );

  /** Empty string → undefined (backend default); otherwise the parsed integer. */
  function parseRobustness(raw: string): number | undefined {
    const trimmed = raw.trim();
    if (trimmed.length === 0) return undefined;
    return Number(trimmed);
  }

  const targetValid = $derived.by((): boolean => {
    const target = form.target;
    switch (target.kind) {
      case "prompt":
        return (
          target.providerId.length > 0 &&
          target.modelId.length > 0 &&
          target.prompt.trim().length > 0
        );
      case "agent":
        return target.agentId.length > 0 && target.modelId.length > 0;
    }
  });

  function validate(): boolean {
    showValidation = true;
    if (form.name.trim().length === 0) return false;
    if (!targetValid) return false;
    if (robustnessError(form.execTimeoutSecs, t("jobs.form.execTimeoutLabel")))
      return false;
    if (robustnessError(form.maxRetries, t("jobs.form.maxRetriesLabel")))
      return false;
    if (robustnessError(form.retryDelaySecs, t("jobs.form.retryDelayLabel")))
      return false;
    return true;
  }

  function backToList() {
    goto("/jobs");
  }

  // Save is not optimistic: failures keep the form and show the error (no ghost
  // cards in the list); success returns to the list page.
  async function handleSave(): Promise<void> {
    if (saving || !validate()) return;
    saving = true;
    saveError = null;
    const data = {
      name: form.name.trim(),
      description: form.description.trim() ? form.description : undefined,
      target: $state.snapshot(form.target) as JobTarget,
      cronExpr: form.cronExpr.trim(),
      timezone: form.timezone,
      enabled: form.enabled,
      execTimeoutSecs: parseRobustness(form.execTimeoutSecs),
      maxRetries: parseRobustness(form.maxRetries),
      retryDelaySecs: parseRobustness(form.retryDelaySecs),
    };
    try {
      if (job?.id) {
        await jobStore.update(job.id, data);
      } else {
        await jobStore.create(data);
      }
      backToList();
    } catch (e) {
      saveError =
        e instanceof AppError
          ? e.message
          : e instanceof Error
            ? e.message
            : t("jobs.form.saveFailed");
    } finally {
      saving = false;
    }
  }
</script>

<!-- Mirrors the Agent editor layout: centered max-w-3xl column of TableGroup cards. -->
<div class="h-full flex flex-col">
  <div class="flex-shrink-0 px-6 pb-4 pt-12">
    <div class="mx-auto w-full max-w-3xl">
      <button
        class="flex items-center gap-2 text-sm text-base-content/70 hover:text-base-content w-fit mb-4"
        onclick={backToList}
      >
        <ArrowLeft size={14} />
        {t("jobs.form.backToList")}
      </button>

      <div class="flex items-center gap-3">
        <div class="min-w-0 flex-1">
          <input
            class="modal-title-input w-full"
            bind:value={form.name}
            placeholder={t("jobs.form.namePlaceholder")}
            aria-invalid={nameError != null}
          />
          {#if nameError}
            <span class="text-xs text-error">{nameError}</span>
          {/if}
        </div>
        <Button
          variant="primary"
          size="sm"
          onclick={handleSave}
          disabled={saving}
          customClass="flex items-center gap-2"
        >
          <Save size={14} />
          {saving
            ? t("jobs.form.saving")
            : job
              ? t("jobs.form.save")
              : t("jobs.form.createAction")}
        </Button>
      </div>

      {#if saveError}
        <div
          class="mt-3 rounded-md bg-error/10 px-3 py-2 text-sm text-error"
        >
          {saveError}
        </div>
      {/if}
    </div>
  </div>

  <div class="flex-1 min-h-0 overflow-y-auto px-6 pb-6">
    <div class="mx-auto flex w-full max-w-3xl flex-col gap-y-4">
      <TableGroup title={t("jobs.form.sectionTarget")}>
        <TableBaseRow>
          <TargetPicker
            bind:target={form.target}
            {providersWithModels}
            {agents}
            {agentsLoading}
            showError={showValidation}
          />
        </TableBaseRow>
      </TableGroup>

      <TableGroup title={t("jobs.form.sectionSchedule")}>
        <TableBaseRow>
          <ScheduleEditor bind:cron={form.cronExpr} />
        </TableBaseRow>
      </TableGroup>

      <TableGroup title={t("jobs.form.sectionAdvanced")}>
        <TableBaseRow
          label={t("jobs.form.execTimeout")}
          description={t("jobs.form.execTimeoutHint")}
          error={execTimeoutError ?? undefined}
        >
          <input
            type="number"
            min="0"
            step="1"
            bind:value={form.execTimeoutSecs}
            placeholder={t("jobs.form.execTimeoutPlaceholder", {
              n: DEFAULT_EXEC_TIMEOUT_SECS,
            })}
            aria-invalid={execTimeoutError != null}
            class="field w-32 px-2.5 py-1.5 text-sm"
            class:is-error={execTimeoutError != null}
          />
        </TableBaseRow>

        <TableBaseRow
          label={t("jobs.form.maxRetries")}
          description={t("jobs.form.maxRetriesHint")}
          error={maxRetriesError ?? undefined}
        >
          <input
            type="number"
            min="0"
            step="1"
            bind:value={form.maxRetries}
            placeholder={t("jobs.form.maxRetriesPlaceholder", {
              n: DEFAULT_MAX_RETRIES,
            })}
            aria-invalid={maxRetriesError != null}
            class="field w-32 px-2.5 py-1.5 text-sm"
            class:is-error={maxRetriesError != null}
          />
        </TableBaseRow>

        <TableBaseRow
          label={t("jobs.form.retryDelay")}
          description={t("jobs.form.retryDelayHint", {
            n: DEFAULT_RETRY_DELAY_SECS,
          })}
          error={retryDelayError ?? undefined}
        >
          <input
            type="number"
            min="0"
            step="1"
            bind:value={form.retryDelaySecs}
            placeholder={t("jobs.form.retryDelayPlaceholder", {
              n: DEFAULT_RETRY_DELAY_SECS,
            })}
            aria-invalid={retryDelayError != null}
            class="field w-32 px-2.5 py-1.5 text-sm"
            class:is-error={retryDelayError != null}
          />
        </TableBaseRow>
      </TableGroup>

      <p class="px-1 text-xs text-base-content/45">
        {t("jobs.form.appClosedNotice")}
      </p>
    </div>
  </div>
</div>
