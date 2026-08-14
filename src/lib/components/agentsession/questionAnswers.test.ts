import { describe, it, expect } from "vitest";
import {
  answersFor,
  buildAnswers,
  canSubmit,
  countAnswered,
  isSelected,
  toggleChoice,
  type ChoiceState,
  type DraftState,
} from "./questionAnswers";
import type { AgentQuestion } from "$lib/types/agentSession";

const single: AgentQuestion = {
  id: "q0",
  header: "Storage",
  question: "Which storage backend?",
  type: "single",
  options: [{ label: "SQLite" }, { label: "In-memory" }],
  required: false,
};

const multiple: AgentQuestion = {
  id: "q1",
  header: "Targets",
  question: "Which platforms?",
  type: "multiple",
  options: [{ label: "macOS" }, { label: "Windows" }, { label: "Linux" }],
  required: false,
};

const text: AgentQuestion = {
  id: "q2",
  header: "Name",
  question: "What should it be called?",
  type: "text",
  options: [],
  required: false,
};

describe("toggleChoice", () => {
  it("keeps at most one label for a single-select", () => {
    let choices: ChoiceState = {};
    choices = toggleChoice(choices, single, "SQLite");
    expect(choices.q0).toEqual(["SQLite"]);
    choices = toggleChoice(choices, single, "In-memory");
    expect(choices.q0).toEqual(["In-memory"]);
  });

  it("clicking the current single-select choice again clears it", () => {
    // A mis-click must be recoverable without dismissing the whole panel.
    let choices = toggleChoice({}, single, "SQLite");
    choices = toggleChoice(choices, single, "SQLite");
    expect(choices.q0).toEqual([]);
  });

  it("accumulates and removes labels independently for a multi-select", () => {
    let choices = toggleChoice({}, multiple, "macOS");
    choices = toggleChoice(choices, multiple, "Linux");
    expect(choices.q1).toEqual(["macOS", "Linux"]);
    choices = toggleChoice(choices, multiple, "macOS");
    expect(choices.q1).toEqual(["Linux"]);
  });

  it("preserves the order the user picked", () => {
    let choices = toggleChoice({}, multiple, "Linux");
    choices = toggleChoice(choices, multiple, "macOS");
    expect(choices.q1).toEqual(["Linux", "macOS"]);
  });

  it("never mutates the state it is given", () => {
    const before: ChoiceState = { q0: ["SQLite"] };
    const after = toggleChoice(before, single, "In-memory");
    expect(before).toEqual({ q0: ["SQLite"] });
    expect(after.q0).toEqual(["In-memory"]);
  });

  it("touches only the question that was clicked", () => {
    const before: ChoiceState = { q1: ["macOS"] };
    const after = toggleChoice(before, single, "SQLite");
    expect(after.q1).toEqual(["macOS"]);
  });
});

describe("isSelected", () => {
  it("reads the selection for the right question", () => {
    const choices: ChoiceState = { q0: ["SQLite"], q1: ["Linux"] };
    expect(isSelected(choices, single, "SQLite")).toBe(true);
    expect(isSelected(choices, single, "In-memory")).toBe(false);
    // Same label text under a different question must not bleed across.
    expect(isSelected(choices, multiple, "SQLite")).toBe(false);
  });
});

describe("answersFor", () => {
  it("trims a text draft", () => {
    expect(answersFor(text, {}, { q2: "  cache layer  " })).toEqual([
      "cache layer",
    ]);
  });

  it("treats a blank or missing text draft as unanswered", () => {
    // Whitespace-only is not an answer; the model must not read it as one.
    expect(answersFor(text, {}, { q2: "   " })).toEqual([]);
    expect(answersFor(text, {}, {})).toEqual([]);
  });

  it("returns the selected labels for a choice question", () => {
    expect(answersFor(multiple, { q1: ["macOS"] }, {})).toEqual(["macOS"]);
    expect(answersFor(multiple, {}, {})).toEqual([]);
  });

  it("ignores a draft left on a choice question and vice versa", () => {
    // The two maps are keyed by the same ids; each kind must read only its own.
    expect(answersFor(single, {}, { q0: "typed here" })).toEqual([]);
    expect(answersFor(text, { q2: ["clicked"] }, {})).toEqual([]);
  });
});

describe("countAnswered", () => {
  it("counts only questions carrying a value", () => {
    const questions = [single, multiple, text];
    expect(countAnswered(questions, {}, {})).toBe(0);
    expect(countAnswered(questions, { q0: ["SQLite"] }, {})).toBe(1);
    expect(
      countAnswered(questions, { q0: ["SQLite"], q1: ["Linux"] }, { q2: "x" }),
    ).toBe(3);
  });

  it("does not count an emptied selection or a blank draft", () => {
    const questions = [single, text];
    expect(countAnswered(questions, { q0: [] }, { q2: "  " })).toBe(0);
  });
});

describe("buildAnswers", () => {
  it("emits one entry per question, in the order asked", () => {
    const answers = buildAnswers(
      [single, multiple, text],
      { q0: ["SQLite"], q1: ["macOS", "Linux"] },
      { q2: "cache layer" },
    );
    expect(answers).toEqual([
      { questionId: "q0", values: ["SQLite"] },
      { questionId: "q1", values: ["macOS", "Linux"] },
      { questionId: "q2", values: ["cache layer"] },
    ]);
  });

  it("keeps unanswered questions with empty values instead of dropping them", () => {
    // Dropping them would let a partial submission read as a full one; the
    // backend renders an empty `values` as "(not answered)".
    const answers = buildAnswers([single, text], { q0: ["SQLite"] }, {});
    expect(answers).toEqual([
      { questionId: "q0", values: ["SQLite"] },
      { questionId: "q2", values: [] },
    ]);
  });

  it("produces an all-empty payload when nothing was touched", () => {
    expect(buildAnswers([single, multiple], {}, {})).toEqual([
      { questionId: "q0", values: [] },
      { questionId: "q1", values: [] },
    ]);
  });
});

describe("canSubmit", () => {
  const requiredText: AgentQuestion = { ...text, required: true };
  const requiredSingle: AgentQuestion = { ...single, required: true };

  it("allows submitting with nothing filled in when no question is required", () => {
    // Optional questions never trap the user; the backend reports the gaps.
    expect(canSubmit([single, multiple, text], {}, {})).toBe(true);
  });

  it("blocks until every required question carries a value", () => {
    expect(canSubmit([requiredSingle, text], {}, {})).toBe(false);
    expect(canSubmit([requiredSingle, text], { q0: ["SQLite"] }, {})).toBe(
      true,
    );
  });

  it("ignores optional questions when deciding", () => {
    // q1/q2 stay blank — only the required q0 matters.
    expect(
      canSubmit([requiredSingle, multiple, text], { q0: ["SQLite"] }, {}),
    ).toBe(true);
  });

  it("treats a whitespace-only draft as not answering a required text question", () => {
    expect(canSubmit([requiredText], {}, { q2: "   " })).toBe(false);
    expect(canSubmit([requiredText], {}, { q2: "ok" })).toBe(true);
  });

  it("requires ALL required questions, not just one", () => {
    expect(
      canSubmit([requiredSingle, requiredText], { q0: ["SQLite"] }, {}),
    ).toBe(false);
    expect(
      canSubmit(
        [requiredSingle, requiredText],
        { q0: ["SQLite"] },
        { q2: "ok" },
      ),
    ).toBe(true);
  });
});
