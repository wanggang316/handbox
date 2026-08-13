/**
 * Pure answer bookkeeping for AgentQuestionPanel.
 *
 * Kept out of the component so the selection rules and the submitted payload
 * are unit-testable in a plain Node environment (the repo's vitest setup has no
 * DOM), and so the panel stays presentation-only.
 *
 * Two maps back the panel: `choices` holds the selected option labels per
 * choice question, `drafts` holds the raw textarea contents per text question.
 * They are separate because a draft must survive mid-typing whitespace — the
 * trim happens only when an answer is read.
 */

import type {
  AgentQuestion,
  AgentQuestionAnswer,
} from "$lib/types/agentSession";

/** Selected option labels, keyed by question id. */
export type ChoiceState = Record<string, string[]>;
/** Raw textarea contents, keyed by question id. */
export type DraftState = Record<string, string>;

/**
 * The values one question currently contributes; an empty array means
 * unanswered. Text answers are trimmed, so a whitespace-only draft counts as no
 * answer rather than as a blank one.
 */
export function answersFor(
  question: AgentQuestion,
  choices: ChoiceState,
  drafts: DraftState,
): string[] {
  if (question.type === "text") {
    const draft = (drafts[question.id] ?? "").trim();
    return draft ? [draft] : [];
  }
  return choices[question.id] ?? [];
}

/**
 * Apply a click on `label`, returning the next choice state (never mutates).
 *
 * Single-select keeps at most one label and allows clicking the selection off
 * again — a mis-click must be recoverable without dismissing the panel.
 * Multi-select toggles each label independently, preserving selection order so
 * the model reads them in the order the user picked.
 */
export function toggleChoice(
  choices: ChoiceState,
  question: AgentQuestion,
  label: string,
): ChoiceState {
  const current = choices[question.id] ?? [];
  const selected = current.includes(label);
  const next =
    question.type === "single"
      ? selected
        ? []
        : [label]
      : selected
        ? current.filter((value) => value !== label)
        : [...current, label];
  return { ...choices, [question.id]: next };
}

export function isSelected(
  choices: ChoiceState,
  question: AgentQuestion,
  label: string,
): boolean {
  return (choices[question.id] ?? []).includes(label);
}

/**
 * Whether the panel may be submitted: every `required` question carries a
 * value. Questions the model left optional never block, so a call with no
 * required question is always submittable — including with nothing filled in,
 * which the backend reports as explicitly unanswered.
 */
export function canSubmit(
  questions: AgentQuestion[],
  choices: ChoiceState,
  drafts: DraftState,
): boolean {
  return questions.every(
    (question) =>
      !question.required || answersFor(question, choices, drafts).length > 0,
  );
}

/** How many questions currently carry at least one value. */
export function countAnswered(
  questions: AgentQuestion[],
  choices: ChoiceState,
  drafts: DraftState,
): number {
  return questions.filter(
    (question) => answersFor(question, choices, drafts).length > 0,
  ).length;
}

/**
 * The `answers` payload for an `answered` response: one entry per question, in
 * the order asked. Unanswered questions are included with an empty `values`
 * rather than dropped — the backend reports them to the model as explicitly
 * unanswered, so a partial submission never reads as a full one.
 */
export function buildAnswers(
  questions: AgentQuestion[],
  choices: ChoiceState,
  drafts: DraftState,
): AgentQuestionAnswer[] {
  return questions.map((question) => ({
    questionId: question.id,
    values: answersFor(question, choices, drafts),
  }));
}
