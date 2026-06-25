<script lang="ts">
  /**
   * Settings · Shortcuts page.
   *
   * The HOTKEY half of the Quick Action settings page: shows the current global
   * hotkey, a recorder to rebind it, and a reset-to-default control. A captured
   * chord is REGISTERED FIRST (so the OS validates it) and only persisted if
   * registration succeeds — on failure the previously-working value is kept and
   * a structured error is shown. This avoids ever persisting a combo the OS
   * rejected with no working hotkey.
   *
   * Load/update mirrors `settings/general/+page.svelte`. Structured so the next
   * feature (`qa-settings-default-model`) can add a second TableGroup cleanly.
   */
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { TableGroup, TableBaseRow } from "$lib/components/ui/table";
  import ShortcutRecorder from "$lib/components/quickaction/ShortcutRecorder.svelte";
  import { settingsState } from "$lib/states";
  import { normalizeError } from "$lib/utils/error";
  import { t } from "$lib/i18n";
  import {
    DEFAULT_ACCELERATOR,
    type AcceleratorInvalidReason,
  } from "$lib/quickaction/accelerator";
  import type { AppError } from "$lib/types";

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
  });

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
</div>
