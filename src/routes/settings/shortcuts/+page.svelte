<script lang="ts">
  /**
   * Settings · Shortcuts page (the Quick Action settings page).
   *
   * HOTKEY section: shows the current global hotkey, a recorder to rebind it,
   * and a reset-to-default control. A captured chord is REGISTERED FIRST (so the
   * OS validates it) and only persisted if registration succeeds — on failure
   * the previously-working value is kept and a structured error is shown. This
   * avoids ever persisting a combo the OS rejected with no working hotkey.
   *
   * DEFAULT-MODEL section: shows (and lets the user pick) the model a fresh
   * overlay summon uses. The choice is persisted IMMEDIATELY on select (no Save
   * step), mirroring `settings/general`'s onChange-immediate pattern. Display is
   * driven by `resolveQuickActionModel(quickAction, getAllModels())`:
   *   - resolved      → show the model name
   *   - dangling      → "unavailable, re-select" placeholder; the stale id is
   *                     LEFT on disk (recoverable if the provider is re-enabled)
   *   - no-default    → "not selected" placeholder
   *   - empty-catalog → guidance to configure a provider under Models
   * The hotkey reset only touches `shortcut`, so it never wipes the model.
   *
   * Load/update mirrors `settings/general/+page.svelte`.
   */
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { goto } from "$app/navigation";
  import { TableGroup, TableBaseRow } from "$lib/components/ui/table";
  import ShortcutRecorder from "$lib/components/quickaction/ShortcutRecorder.svelte";
  import ChatModelSelectButton from "$lib/components/chat/ChatModelSelectButton.svelte";
  import { settingsState, providerActions } from "$lib/states";
  import { getAllModels } from "$lib/states/provider.svelte";
  import { normalizeError } from "$lib/utils/error";
  import { t } from "$lib/i18n";
  import {
    DEFAULT_ACCELERATOR,
    type AcceleratorInvalidReason,
  } from "$lib/quickaction/accelerator";
  import { resolveQuickActionModel } from "$lib/quickaction/resolveModel";
  import type { AppError } from "$lib/types";
  import type { ModelWithProvider } from "$lib/types/provider";

  // The currently configured hotkey (falls back to the default when unset).
  let shortcut = $state(DEFAULT_ACCELERATOR);
  // Field-level validation guidance (invalid chord); cleared on a fresh attempt.
  let invalidReason = $state<AcceleratorInvalidReason | null>(null);
  // Structured OS-registration error (code/message/hint); shown verbatim.
  let registerError = $state<AppError | null>(null);
  // Gate concurrent register/persist round-trips.
  let busy = $state(false);

  onMount(async () => {
    try {
      await settingsState.loadSettings();
      const configured = settingsState.settings?.quickAction?.shortcut;
      if (configured) {
        shortcut = configured;
      }
    } catch (error) {
      console.error("加载快捷键设置失败:", error);
    }
    try {
      // Catalog needed to resolve / detect-dangling the default model display.
      await providerActions.loadProvidersWithModels();
    } catch (error) {
      console.error("加载模型目录失败:", error);
    }
  });

  // Resolve the persisted default against the live catalog. Reactive on both
  // the settings slice (changes when we persist a pick) and the catalog (loaded
  // in onMount). Decides which display state the model row shows.
  const modelResolution = $derived(
    resolveQuickActionModel(
      settingsState.settings?.quickAction,
      getAllModels(),
    ),
  );

  // The selected model passed to ChatModelSelectButton: only the resolved (=
  // runnable) model, so a dangling/empty default shows the button placeholder
  // instead of a stale name. The row-level placeholders handle the rest.
  const selectedModel = $derived<ModelWithProvider | null>(
    modelResolution.available ? modelResolution.model : null,
  );

  /**
   * A default model was picked: persist `modelId`/`providerId` immediately (no
   * Save). The Model catalog uses snake_case `provider_id`; settings store
   * camelCase `providerId`.
   */
  async function handleModelSelect(model: ModelWithProvider): Promise<void> {
    try {
      await settingsState.updateSettings({
        section: "quickAction",
        data: { modelId: model.id, providerId: model.provider_id },
      });
    } catch (error) {
      console.error("更新快捷动作默认模型失败:", error);
    }
  }

  /** Jump to the model settings to enable a provider (empty-catalog guidance). */
  function openModelSettings(): void {
    void goto("/settings/models");
  }

  /** Map a pure-helper invalid reason to its localized guidance string. */
  function invalidMessage(reason: AcceleratorInvalidReason): string {
    switch (reason) {
      case "modifier-only":
        return t("quickaction.shortcut.invalid.modifierOnly");
      case "no-modifier":
        return t("quickaction.shortcut.invalid.noModifier");
      case "unsupported-key":
        return t("quickaction.shortcut.invalid.unsupportedKey");
    }
  }

  /** Combined field-level error text (validation OR registration), if any. */
  const fieldError = $derived(
    invalidReason !== null
      ? invalidMessage(invalidReason)
      : registerError !== null
        ? registerError.message
        : undefined,
  );

  /**
   * A valid chord was captured: register it live FIRST so the OS validates the
   * combo, then persist only on success. On failure keep the prior value and
   * surface the structured error.
   */
  async function applyAccelerator(next: string): Promise<void> {
    invalidReason = null;
    registerError = null;
    if (busy || next === shortcut) {
      return;
    }
    busy = true;
    try {
      // Register first — this validates against the OS and swaps the live combo
      // (unregister-old-before-register is handled by the command).
      await invoke("quick_action_register_shortcut", { accelerator: next });
      // Registration succeeded → persist the new value.
      await settingsState.updateSettings({
        section: "quickAction",
        data: { shortcut: next },
      });
      shortcut = next;
    } catch (error) {
      // Keep the previously-working value; surface the structured error.
      registerError = normalizeError(
        error,
        t("quickaction.shortcut.registerFailed"),
      );
    } finally {
      busy = false;
    }
  }

  /** Recorder captured a valid chord. */
  function handleCapture(accelerator: string): void {
    void applyAccelerator(accelerator);
  }

  /** Recorder captured an invalid chord: show guidance, persist nothing. */
  function handleInvalid(reason: AcceleratorInvalidReason): void {
    registerError = null;
    invalidReason = reason;
  }

  /** Reset to the default hotkey: register + persist via the same path. */
  function handleReset(): void {
    void applyAccelerator(DEFAULT_ACCELERATOR);
  }
</script>

<div class="mt-8 p-6 pr-8 flex flex-col gap-y-4">
  <TableGroup title={t("quickaction.shortcut.title")}>
    <TableBaseRow
      label={t("quickaction.shortcut.label")}
      layout="vertical"
      helpText={t("quickaction.shortcut.hint")}
      error={fieldError}
    >
      <div class="flex items-center justify-between gap-3 mt-2">
        <ShortcutRecorder
          value={shortcut}
          onCapture={handleCapture}
          onInvalid={handleInvalid}
          disabled={busy}
        />
        <button
          type="button"
          class="text-sm text-base-content/70 hover:text-base-content transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          disabled={busy || shortcut === DEFAULT_ACCELERATOR}
          onclick={handleReset}
        >
          {t("quickaction.shortcut.reset")}
        </button>
      </div>
      {#if registerError?.hint}
        <p class="text-xs text-base-content/60 mt-1">{registerError.hint}</p>
      {/if}
    </TableBaseRow>
  </TableGroup>

  <TableGroup title={t("quickaction.model.title")}>
    <TableBaseRow
      label={t("quickaction.model.label")}
      layout="vertical"
      helpText={t("quickaction.model.hint")}
    >
      {#if modelResolution.available || modelResolution.reason !== "empty-catalog"}
        <div class="flex items-center justify-between gap-3 mt-2">
          <ChatModelSelectButton
            {selectedModel}
            onModelSelect={handleModelSelect}
          />
          {#if !modelResolution.available}
            <span class="text-xs text-base-content/60">
              {modelResolution.reason === "dangling-default"
                ? t("quickaction.model.unavailable")
                : t("quickaction.model.none")}
            </span>
          {/if}
        </div>
      {:else}
        <div class="flex items-center justify-between gap-3 mt-2">
          <p class="text-sm text-base-content/70">
            {t("quickaction.model.emptyCatalog")}
          </p>
          <button
            type="button"
            class="text-sm text-base-content/70 hover:text-base-content transition-colors whitespace-nowrap"
            onclick={openModelSettings}
          >
            {t("quickaction.model.openModels")}
          </button>
        </div>
      {/if}
    </TableBaseRow>
  </TableGroup>
</div>
