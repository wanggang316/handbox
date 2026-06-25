/**
 * Pure keyboard-accelerator helpers for the Quick Action hotkey recorder.
 *
 * Bridges the browser's `KeyboardEvent` to the accelerator-string grammar the
 * Rust side parses (`tauri-plugin-global-shortcut` → `global-hotkey`'s
 * `HotKey::from_str`). That grammar is `Modifier+...+Key` joined by `+`, where:
 * - modifiers (case-insensitive): `CmdOrCtrl`, `Cmd`/`Command`/`Super`,
 *   `Ctrl`/`Control`, `Alt`/`Option`, `Shift`;
 * - keys are `KeyboardEvent.code`-style tokens — `KeyK`, `Digit1`, `Space`,
 *   `Enter`, `ArrowUp`, `F1`, `Backquote`, … — which map 1:1 onto the plugin's
 *   `parse_key` aliases.
 *
 * The default accelerator is {@link DEFAULT_ACCELERATOR} (`CmdOrCtrl+Shift+Space`).
 *
 * Kept PURE (a `KeyboardEvent`-shaped input in, a string/boolean out) so the
 * build+validate logic is unit-testable without a DOM or the global-shortcut
 * plugin. See `accelerator.test.ts`.
 */

/** The app-wide default Quick Action hotkey; must parse on the Rust side. */
export const DEFAULT_ACCELERATOR = "CmdOrCtrl+Shift+Space";

/**
 * The subset of `KeyboardEvent` the builder reads. Accepting a structural type
 * (rather than the DOM `KeyboardEvent`) keeps the helper pure and trivially
 * testable with plain objects.
 */
export interface AcceleratorKeyEvent {
  /** Physical key code, e.g. `"KeyK"`, `"Space"`, `"Digit1"`, `"MetaLeft"`. */
  readonly code: string;
  readonly metaKey: boolean;
  readonly ctrlKey: boolean;
  readonly altKey: boolean;
  readonly shiftKey: boolean;
}

/**
 * `KeyboardEvent.code` values that are themselves modifier keys. A chord whose
 * main key is one of these is "modifier-only" and therefore invalid.
 */
const MODIFIER_CODES: ReadonlySet<string> = new Set([
  "MetaLeft",
  "MetaRight",
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  // Some layouts surface a bare "OSLeft"/"OSRight"; treat as modifiers too.
  "OSLeft",
  "OSRight",
]);

/**
 * Result of inspecting a keyboard event for a usable chord.
 *
 * `valid` chords carry the canonical accelerator string; invalid ones carry a
 * machine-readable {@link AcceleratorInvalidReason} the UI maps to localized
 * guidance.
 */
export type AcceleratorResult =
  | { valid: true; accelerator: string }
  | { valid: false; reason: AcceleratorInvalidReason };

/** Why a captured chord cannot be used as a global hotkey. */
export type AcceleratorInvalidReason =
  | "modifier-only" // only modifier keys held, no main key (e.g. just Cmd)
  | "no-modifier" // a bare key with no modifier (e.g. just K)
  | "unsupported-key"; // the main key has no accelerator token

/**
 * Map a `KeyboardEvent.code` to the accelerator key token the Rust parser
 * accepts. The plugin grammar accepts the `code` token verbatim for the keys we
 * allow (letters `KeyA`–`KeyZ`, digits `Digit0`–`Digit9`, `F1`–`F24`, arrows,
 * `Space`/`Enter`/`Tab`/punctuation/etc.), so this is an identity pass plus a
 * `null` for codes the parser does not understand.
 *
 * Returns `null` for modifier codes and anything unsupported.
 */
export function keyToken(code: string): string | null {
  if (!code || MODIFIER_CODES.has(code)) {
    return null;
  }
  return SUPPORTED_KEY_CODES.has(code) ? code : null;
}

/**
 * The set of `KeyboardEvent.code` values the Rust accelerator parser accepts as
 * a main key. Mirrors `global-hotkey`'s `parse_key` `code`-form aliases.
 */
const SUPPORTED_KEY_CODES: ReadonlySet<string> = new Set([
  // Letters.
  ...Array.from({ length: 26 }, (_, i) => `Key${String.fromCharCode(65 + i)}`),
  // Digits (top-row).
  ...Array.from({ length: 10 }, (_, i) => `Digit${i}`),
  // Numpad digits.
  ...Array.from({ length: 10 }, (_, i) => `Numpad${i}`),
  // Function keys.
  ...Array.from({ length: 24 }, (_, i) => `F${i + 1}`),
  // Whitespace / editing.
  "Space",
  "Enter",
  "Tab",
  "Backspace",
  "Delete",
  "CapsLock",
  // Navigation.
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "Home",
  "End",
  "PageUp",
  "PageDown",
  "Insert",
  "Escape",
  // Punctuation / symbols.
  "Backquote",
  "Backslash",
  "BracketLeft",
  "BracketRight",
  "Comma",
  "Equal",
  "Minus",
  "Period",
  "Quote",
  "Semicolon",
  "Slash",
  // Numpad operators.
  "NumpadAdd",
  "NumpadSubtract",
  "NumpadMultiply",
  "NumpadDivide",
  "NumpadDecimal",
  "NumpadEnter",
  "NumpadEqual",
  // Misc.
  "PrintScreen",
  "ScrollLock",
  "NumLock",
  "Pause",
]);

/**
 * Build the ordered modifier tokens held in an event, in canonical order
 * (`CmdOrCtrl`, `Alt`, `Shift`) so the produced string mirrors the default
 * `CmdOrCtrl+Shift+Space`.
 *
 * `metaKey` (⌘ on macOS) and `ctrlKey` both collapse to `CmdOrCtrl` so the
 * accelerator stays portable; the OS-level parser resolves it per-platform.
 */
function modifierTokens(event: AcceleratorKeyEvent): string[] {
  const tokens: string[] = [];
  if (event.metaKey || event.ctrlKey) {
    tokens.push("CmdOrCtrl");
  }
  if (event.altKey) {
    tokens.push("Alt");
  }
  if (event.shiftKey) {
    tokens.push("Shift");
  }
  return tokens;
}

/**
 * Inspect a keyboard event and return either the canonical accelerator string
 * or the reason it is unusable.
 *
 * A valid chord requires at least one modifier AND a non-modifier main key.
 */
export function buildAccelerator(event: AcceleratorKeyEvent): AcceleratorResult {
  const main = keyToken(event.code);
  const mods = modifierTokens(event);

  if (main === null) {
    // No usable main key. Distinguish "they only held modifiers" from "we
    // don't support this key" so the UI can give precise guidance.
    return {
      valid: false,
      reason: MODIFIER_CODES.has(event.code)
        ? "modifier-only"
        : "unsupported-key",
    };
  }

  if (mods.length === 0) {
    return { valid: false, reason: "no-modifier" };
  }

  return { valid: true, accelerator: [...mods, main].join("+") };
}

/**
 * Validate an already-built accelerator string: it must contain at least one
 * recognized modifier token AND a trailing non-modifier key token.
 *
 * Used to guard a value before persist/register without re-deriving it from an
 * event (e.g. the configured default, or a value round-tripped from settings).
 */
export function validateAccelerator(accelerator: string): boolean {
  const tokens = accelerator.split("+").map((t) => t.trim());
  if (tokens.length < 2 || tokens.some((t) => t.length === 0)) {
    return false;
  }

  const last = tokens[tokens.length - 1];
  const mods = tokens.slice(0, -1);

  // Every leading token must be a known modifier, the last must be a known key,
  // and the last must NOT itself be a modifier.
  if (mods.length === 0 || mods.some((m) => !isModifierToken(m))) {
    return false;
  }
  return !isModifierToken(last) && SUPPORTED_KEY_CODES.has(last);
}

/** Whether a token (case-insensitive) is a recognized modifier. */
function isModifierToken(token: string): boolean {
  switch (token.toUpperCase()) {
    case "CMDORCTRL":
    case "COMMANDORCONTROL":
    case "COMMANDORCTRL":
    case "CMDORCONTROL":
    case "CMD":
    case "COMMAND":
    case "SUPER":
    case "CTRL":
    case "CONTROL":
    case "ALT":
    case "OPTION":
    case "SHIFT":
      return true;
    default:
      return false;
  }
}

/**
 * Render an accelerator string for display, e.g. `CmdOrCtrl+Shift+Space` →
 * `⌘ ⇧ Space` on macOS. Pure + presentation-only; the canonical string stays
 * the source of truth for persist/register.
 */
export function formatAccelerator(accelerator: string): string {
  return accelerator
    .split("+")
    .map((token) => SYMBOLS[token.toUpperCase()] ?? humanizeKey(token))
    .join(" ");
}

/** Display symbols for modifiers (macOS glyphs). */
const SYMBOLS: Record<string, string> = {
  CMDORCTRL: "⌘",
  COMMANDORCONTROL: "⌘",
  CMD: "⌘",
  COMMAND: "⌘",
  SUPER: "⌘",
  CTRL: "⌃",
  CONTROL: "⌃",
  ALT: "⌥",
  OPTION: "⌥",
  SHIFT: "⇧",
};

/** Strip the `Key`/`Digit` prefix from key tokens for a friendlier label. */
function humanizeKey(token: string): string {
  if (token.startsWith("Key") && token.length === 4) {
    return token.slice(3);
  }
  if (token.startsWith("Digit") && token.length === 6) {
    return token.slice(5);
  }
  return token;
}
