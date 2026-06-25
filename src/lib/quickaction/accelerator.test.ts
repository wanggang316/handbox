/**
 * Unit tests for the pure Quick Action accelerator helper.
 *
 * Pure TypeScript — the helper takes a `KeyboardEvent`-shaped object and returns
 * a string/boolean, importing no `.svelte` module, so this suite runs under the
 * plain-Node Vitest environment.
 *
 * Covers the validation assertions the milestone gate cannot exercise headlessly:
 * - VAL-SETTINGS-005: a modifier-only chord (just Cmd) is rejected.
 * - VAL-SETTINGS-018: a bare key with no modifier (just K) is rejected.
 * Plus the happy path (modifier+key → valid, correct accelerator string format).
 */

import { describe, it, expect } from "vitest";
import {
  buildAccelerator,
  validateAccelerator,
  formatAccelerator,
  keyToken,
  DEFAULT_ACCELERATOR,
  type AcceleratorKeyEvent,
} from "./accelerator";

/** Build an event with no modifiers held; override per case. */
function makeEvent(
  overrides: Partial<AcceleratorKeyEvent> & Pick<AcceleratorKeyEvent, "code">,
): AcceleratorKeyEvent {
  return {
    metaKey: false,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    ...overrides,
  };
}

describe("buildAccelerator", () => {
  it("builds a valid accelerator from a modifier+key chord", () => {
    const result = buildAccelerator(
      makeEvent({ code: "KeyK", metaKey: true }),
    );

    expect(result).toEqual({ valid: true, accelerator: "CmdOrCtrl+KeyK" });
  });

  it("emits modifiers in canonical order (CmdOrCtrl, Alt, Shift) matching the default", () => {
    const result = buildAccelerator(
      makeEvent({
        code: "Space",
        metaKey: true,
        shiftKey: true,
        altKey: true,
      }),
    );

    expect(result).toEqual({
      valid: true,
      accelerator: "CmdOrCtrl+Alt+Shift+Space",
    });
  });

  it("collapses both Meta and Ctrl to the portable CmdOrCtrl token", () => {
    const metaChord = buildAccelerator(
      makeEvent({ code: "Space", metaKey: true, shiftKey: true }),
    );
    const ctrlChord = buildAccelerator(
      makeEvent({ code: "Space", ctrlKey: true, shiftKey: true }),
    );

    expect(metaChord).toEqual({
      valid: true,
      accelerator: "CmdOrCtrl+Shift+Space",
    });
    // Both produce the same portable accelerator (and it is the default).
    expect(ctrlChord).toEqual(metaChord);
    if (metaChord.valid) {
      expect(metaChord.accelerator).toBe(DEFAULT_ACCELERATOR);
    }
  });

  it("rejects a modifier-only chord (just Cmd) — VAL-SETTINGS-005", () => {
    const result = buildAccelerator(
      makeEvent({ code: "MetaLeft", metaKey: true }),
    );

    expect(result).toEqual({ valid: false, reason: "modifier-only" });
  });

  it("rejects every bare modifier key code as modifier-only", () => {
    for (const code of [
      "MetaLeft",
      "MetaRight",
      "ControlLeft",
      "ControlRight",
      "AltLeft",
      "AltRight",
      "ShiftLeft",
      "ShiftRight",
    ]) {
      expect(
        buildAccelerator(makeEvent({ code, shiftKey: true, metaKey: true })),
      ).toEqual({ valid: false, reason: "modifier-only" });
    }
  });

  it("rejects a bare key with no modifier (just K) — VAL-SETTINGS-018", () => {
    const result = buildAccelerator(makeEvent({ code: "KeyK" }));

    expect(result).toEqual({ valid: false, reason: "no-modifier" });
  });

  it("rejects an unsupported main key with a distinct reason", () => {
    const result = buildAccelerator(
      makeEvent({ code: "MediaPlayPause", metaKey: true }),
    );

    expect(result).toEqual({ valid: false, reason: "unsupported-key" });
  });

  it("produces a string parseable by validateAccelerator", () => {
    const result = buildAccelerator(
      makeEvent({ code: "Digit1", metaKey: true, shiftKey: true }),
    );

    expect(result.valid).toBe(true);
    if (result.valid) {
      expect(result.accelerator).toBe("CmdOrCtrl+Shift+Digit1");
      expect(validateAccelerator(result.accelerator)).toBe(true);
    }
  });
});

describe("keyToken", () => {
  it("passes through supported key codes verbatim", () => {
    expect(keyToken("KeyK")).toBe("KeyK");
    expect(keyToken("Space")).toBe("Space");
    expect(keyToken("Digit1")).toBe("Digit1");
    expect(keyToken("F5")).toBe("F5");
    expect(keyToken("ArrowUp")).toBe("ArrowUp");
  });

  it("returns null for modifier codes and unsupported keys", () => {
    expect(keyToken("MetaLeft")).toBeNull();
    expect(keyToken("ShiftRight")).toBeNull();
    expect(keyToken("MediaPlayPause")).toBeNull();
    expect(keyToken("")).toBeNull();
  });
});

describe("validateAccelerator", () => {
  it("accepts the configured default", () => {
    expect(validateAccelerator(DEFAULT_ACCELERATOR)).toBe(true);
  });

  it("accepts a modifier+key string", () => {
    expect(validateAccelerator("CmdOrCtrl+KeyK")).toBe(true);
    expect(validateAccelerator("Alt+Shift+ArrowUp")).toBe(true);
    expect(validateAccelerator("Ctrl+Digit1")).toBe(true);
  });

  it("rejects a modifier-only string — VAL-SETTINGS-005", () => {
    expect(validateAccelerator("CmdOrCtrl")).toBe(false);
    expect(validateAccelerator("CmdOrCtrl+Shift")).toBe(false);
  });

  it("rejects a bare key string — VAL-SETTINGS-018", () => {
    expect(validateAccelerator("KeyK")).toBe(false);
    expect(validateAccelerator("Space")).toBe(false);
  });

  it("rejects empty tokens and malformed input", () => {
    expect(validateAccelerator("")).toBe(false);
    expect(validateAccelerator("CmdOrCtrl+")).toBe(false);
    expect(validateAccelerator("+KeyK")).toBe(false);
    expect(validateAccelerator("CmdOrCtrl+NotAKey")).toBe(false);
  });
});

describe("formatAccelerator", () => {
  it("renders modifiers as glyphs and humanizes key tokens", () => {
    expect(formatAccelerator("CmdOrCtrl+Shift+Space")).toBe("⌘ ⇧ Space");
    expect(formatAccelerator("CmdOrCtrl+KeyK")).toBe("⌘ K");
    expect(formatAccelerator("Alt+Digit1")).toBe("⌥ 1");
  });
});
