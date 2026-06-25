<script lang="ts">
  /**
   * Keyboard-shortcut recorder for the Quick Action global hotkey.
   *
   * A focusable button that, when activated (click / Enter / Space), enters a
   * capture state: the next chord pressed is turned into an accelerator via the
   * pure {@link buildAccelerator} helper. A valid chord is emitted through
   * {@link Props.onCapture}; an invalid one surfaces guidance via
   * {@link Props.onInvalid} and stays in capture. Esc or blur cancels — the
   * displayed value reverts to {@link Props.value} and nothing is emitted.
   *
   * Pure capture/validate logic lives in `$lib/quickaction/accelerator`; this
   * component only owns the focus/recording UX. Keyboard-reachable (Tab → focus,
   * Enter/Space → record, capture, Tab/Esc → leave) per VAL-SETTINGS-017.
   */
  import { t } from "$lib/i18n";
  import {
    buildAccelerator,
    formatAccelerator,
    type AcceleratorInvalidReason,
  } from "$lib/quickaction/accelerator";

  interface Props {
    /** The current persisted accelerator (canonical string). */
    value: string;
    /** Called with a valid new accelerator string when a chord is captured. */
    onCapture: (accelerator: string) => void;
    /** Called with the reason a captured chord was rejected. */
    onInvalid?: (reason: AcceleratorInvalidReason) => void;
    /** Disable interaction while a register/persist round-trip is in flight. */
    disabled?: boolean;
  }

  let { value, onCapture, onInvalid, disabled = false }: Props = $props();

  let recording = $state(false);
  let buttonEl = $state<HTMLButtonElement | null>(null);

  const display = $derived(recording ? "" : formatAccelerator(value));

  function startRecording() {
    if (disabled) return;
    recording = true;
  }

  function stopRecording() {
    recording = false;
  }

  /**
   * While recording, intercept every keydown. Modifier-only presses keep
   * recording (the user is mid-chord); Esc cancels; any other key ends the
   * capture with a build+validate of the full chord.
   */
  function handleKeydown(event: KeyboardEvent) {
    if (!recording) {
      // Not recording: Enter/Space activates the recorder (keyboard-reachable).
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        startRecording();
      }
      return;
    }

    // Recording: swallow the chord so it never leaks to the page/shortcuts.
    event.preventDefault();
    event.stopPropagation();

    if (event.key === "Escape") {
      stopRecording();
      return;
    }

    // Bare modifier keydowns (just ⌘ held, no main key yet) keep recording so
    // the user can build a chord; the result is only evaluated on a real key.
    if (isModifierKey(event)) {
      return;
    }

    const result = buildAccelerator(event);
    if (result.valid) {
      stopRecording();
      onCapture(result.accelerator);
    } else if (result.reason === "modifier-only") {
      // Should not happen here (modifier keys are filtered above), but guard.
      return;
    } else {
      // no-modifier / unsupported-key: surface guidance, stay in capture.
      onInvalid?.(result.reason);
    }
  }

  /** Whether the pressed key is itself a modifier (no main key yet). */
  function isModifierKey(event: KeyboardEvent): boolean {
    return (
      event.key === "Meta" ||
      event.key === "Control" ||
      event.key === "Alt" ||
      event.key === "Shift"
    );
  }

  /** Cancel an in-progress capture if focus leaves the recorder. */
  function handleBlur() {
    if (recording) {
      stopRecording();
    }
  }
</script>

<button
  bind:this={buttonEl}
  type="button"
  class="inline-flex items-center justify-center min-w-32 px-3 py-1.5 rounded-lg border text-sm font-mono tabular-nums transition-colors
    {recording
    ? 'border-primary text-primary bg-primary/5 animate-pulse'
    : 'border-[var(--hairline)] text-base-content hover:bg-base-300'}
    {disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}"
  aria-label={t("quickaction.shortcut.label")}
  aria-pressed={recording}
  {disabled}
  onclick={startRecording}
  onkeydown={handleKeydown}
  onblur={handleBlur}
>
  {#if recording}
    <span class="text-base-content/60">{t("quickaction.shortcut.recording")}</span>
  {:else}
    {display}
  {/if}
</button>
