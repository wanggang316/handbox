<script lang="ts">
  /**
   * The `ask_question` answering surface: slides up out of the composer while
   * the model's turn is parked, and hands the answers back through `onRespond`.
   *
   * Deliberately NOT a modal (unlike AgentApprovalModal): a question is not a
   * security gate, so the transcript must stay readable and scrollable while the
   * user decides. The panel therefore docks above the composer instead of
   * covering the conversation, and dismissing it is a first-class outcome rather
   * than a fail-closed denial.
   *
   * ONE QUESTION PER CARD, swiped horizontally: a vertical stack of every
   * question turns a 3-question call into a wall of form above the composer, and
   * the panel is deliberately short (it must not bury the transcript). Cards keep
   * each question at full width with room for option descriptions, and make
   * "how many are left" explicit instead of implied by scroll position.
   *
   * Answer state is local and keyed by question id, so it survives moving between
   * cards. The mount site wraps this in `{#key requestId}`, so a new request
   * always starts from a blank panel instead of inheriting the previous one's
   * selections.
   */
  import {
    MessageCircleQuestionMark,
    Check,
    ChevronLeft,
    ChevronRight,
  } from "@lucide/svelte";
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import Button from "$lib/components/ui/Button.svelte";
  import { t } from "$lib/i18n";
  import {
    buildAnswers,
    canSubmit as canSubmitAnswers,
    countAnswered,
    isSelected,
    toggleChoice,
    type ChoiceState,
    type DraftState,
  } from "./questionAnswers";
  import type {
    AgentQuestion,
    AgentQuestionRequest,
    AgentQuestionResponse,
  } from "$lib/types/agentSession";

  interface Props {
    /** The parked request; every question becomes one card, in order. */
    request: AgentQuestionRequest;
    /**
     * Answer callback. It passes back the request THIS panel displays, so the
     * caller responds to that exact requestId (shown == answered, no re-fetch
     * race). "answered" hands the values to the model as the tool result;
     * "dismissed" tells it the user wants to keep talking instead.
     */
    onRespond: (
      request: AgentQuestionRequest,
      response: AgentQuestionResponse,
    ) => void;
  }

  let { request, onRespond }: Props = $props();

  // Selected option labels per choice question, and the raw draft per text
  // question. The rules operating on them live in `./questionAnswers` so they
  // stay unit-testable; this component only holds and renders the state.
  let choices = $state<ChoiceState>({});
  let drafts = $state<DraftState>({});

  /** Index of the visible card. */
  let index = $state(0);

  let panelRef = $state<HTMLElement | null>(null);

  const total = $derived(request.questions.length);
  const multi = $derived(total > 1);
  const atFirst = $derived(index === 0);
  const atLast = $derived(index >= total - 1);

  // The composer is disabled while a request is pending, so focusing the panel
  // steals nothing — and it makes the keyboard shortcuts work without a click.
  $effect(() => {
    panelRef?.focus({ preventScroll: true });
  });

  function goTo(next: number): void {
    index = Math.min(Math.max(next, 0), total - 1);
  }

  // Only `required` questions gate submission; everything else may be left
  // blank and comes back to the model marked unanswered. Submit stays reachable
  // from EVERY card — the cards are a layout, not a wizard to walk to the end.
  const canSubmit = $derived(canSubmitAnswers(request.questions, choices, drafts));

  function submit(): void {
    if (!canSubmit) return;
    onRespond(request, {
      kind: "answered",
      answers: buildAnswers(request.questions, choices, drafts),
    });
  }

  function dismiss(): void {
    onRespond(request, { kind: "dismissed" });
  }

  /**
   * Escape dismisses (the panel is not fail-closed, so leaving it IS the
   * "keep talking" answer), Cmd/Ctrl+Enter submits, and ←/→ move between cards.
   *
   * Bound at the window rather than on the panel: the composer is disabled while
   * a request is pending and the panel is only mounted then, so these chords have
   * no other owner to compete with, and they keep working if focus lands outside
   * the panel.
   */
  function handleKeydown(event: KeyboardEvent): void {
    // A modal opened over the panel (model picker, agent form) traps focus, so
    // an active element outside the panel means the chord belongs to that
    // dialog — Escape must close it, not silently dismiss the questions behind
    // it. Focus on <body> still counts as ours (the user clicked the
    // transcript).
    const active = document.activeElement;
    const ours =
      active === null ||
      active === document.body ||
      !!panelRef?.contains(active);
    if (!ours) {
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      dismiss();
      return;
    }
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      submit();
      return;
    }
    // Arrows belong to the caret while a text answer is being typed.
    const typing =
      active instanceof HTMLTextAreaElement || active instanceof HTMLInputElement;
    if (multi && !typing && (event.key === "ArrowLeft" || event.key === "ArrowRight")) {
      event.preventDefault();
      goTo(index + (event.key === "ArrowRight" ? 1 : -1));
    }
  }

  function kindLabel(question: AgentQuestion): string {
    if (question.type === "single") return t("agent.question.kindSingle");
    if (question.type === "multiple") return t("agent.question.kindMultiple");
    return t("agent.question.kindText");
  }

  /** Whether a card carries an answer — drives the dot indicator's filled state. */
  function isAnswered(question: AgentQuestion): boolean {
    return countAnswered([question], choices, drafts) > 0;
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Docked above the composer; flies up out of it so the panel reads as coming
     from the input, not as a layer dropped over the conversation. -->
<section
  bind:this={panelRef}
  tabindex="-1"
  aria-label={t("agent.question.panelAria")}
  class="mb-2 w-full overflow-hidden rounded-lg border border-[var(--hairline)] bg-[var(--bg-page)] shadow-lg outline-none"
  transition:fly={{ y: 24, duration: 220, easing: cubicOut }}
>
  <div
    class="flex items-center gap-2 border-b border-[var(--hairline)] px-4 py-2.5"
  >
    <MessageCircleQuestionMark size={15} class="shrink-0 text-primary" />
    <span class="text-[13px] font-medium text-base-content"
      >{t("agent.question.title")}</span
    >
    {#if multi}
      <span class="text-[11px] text-base-content/45"
        >{t("agent.question.progress", {
          current: index + 1,
          total,
        })}</span
      >
    {/if}
  </div>

  <!-- Slide viewport. The cards share one grid cell so the viewport is as tall
       as the TALLEST card: moving between a 4-option card and a textarea card
       must not make the panel (and the composer under it) jump. -->
  <div class="relative">
    <div class="grid overflow-hidden">
      {#each request.questions as question, i (question.id)}
        <div
          class="col-start-1 row-start-1 max-h-[38vh] overflow-y-auto px-4 py-3 transition-[transform,opacity] duration-300 ease-out motion-reduce:transition-none"
          style="transform: translateX({(i - index) * 100}%); opacity: {i ===
          index
            ? 1
            : 0};"
          aria-hidden={i !== index}
          inert={i !== index}
        >
          <div class="mb-2 flex items-baseline gap-2">
            <span
              class="shrink-0 rounded bg-base-300/60 px-1.5 py-0.5 text-[10px] font-medium text-base-content/70"
              >{question.header}</span
            >
            <span class="text-[13px] leading-relaxed text-base-content"
              >{question.question}</span
            >
            <span class="ml-auto flex shrink-0 items-center gap-1 text-[10px]">
              {#if question.required}
                <span class="text-error" title={t("agent.question.required")}
                  >{t("agent.question.required")}</span
                >
              {/if}
              <span class="text-base-content/40">{kindLabel(question)}</span>
            </span>
          </div>

          {#if question.type === "text"}
            <textarea
              bind:value={drafts[question.id]}
              rows="3"
              placeholder={t("agent.question.textPlaceholder")}
              class="w-full resize-none rounded-md border border-[var(--hairline)] bg-base-100 px-2.5 py-2 text-[13px] text-base-content/90 outline-none transition-colors focus:border-primary"
            ></textarea>
          {:else}
            <!-- role reflects the selection semantics so screen readers announce
                 "one of" vs "any of" without reading the kind label. -->
            <div
              role={question.type === "single" ? "radiogroup" : "group"}
              aria-label={question.question}
              class="flex flex-col gap-1.5"
            >
              {#each question.options as option (option.label)}
                {@const selected = isSelected(choices, question, option.label)}
                <button
                  type="button"
                  role={question.type === "single" ? "radio" : "checkbox"}
                  aria-checked={selected}
                  class={`flex w-full items-start gap-2.5 rounded-md border px-2.5 py-2 text-left transition-colors ${
                    selected
                      ? "border-primary bg-primary/10"
                      : "border-[var(--hairline)] hover:bg-base-300/40"
                  }`}
                  onclick={() =>
                    (choices = toggleChoice(choices, question, option.label))}
                >
                  <span
                    class={`mt-[2px] flex h-[14px] w-[14px] shrink-0 items-center justify-center border transition-colors ${
                      question.type === "single"
                        ? "rounded-full"
                        : "rounded-[4px]"
                    } ${
                      selected
                        ? "border-primary bg-primary text-primary-content"
                        : "border-base-content/30"
                    }`}
                  >
                    {#if selected}
                      <Check size={10} strokeWidth={3} />
                    {/if}
                  </span>
                  <span class="min-w-0">
                    <span
                      class="block text-[13px] leading-snug text-base-content"
                      >{option.label}</span
                    >
                    {#if option.description}
                      <span
                        class="mt-0.5 block text-[11px] leading-snug text-base-content/55"
                        >{option.description}</span
                      >
                    {/if}
                  </span>
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {/each}
    </div>

  </div>

  <div
    class="flex items-center justify-between gap-3 border-t border-[var(--hairline)] px-4 py-2.5"
  >
    <div class="flex min-w-0 items-center gap-3">
      {#if multi}
        <!-- Prev/next live in the footer rather than floating over the card:
             a fixed, predictable spot beats controls that overlay the content
             and move with the card's height. Disabled (not hidden) at the ends
             so the row never reflows mid-navigation. -->
        <div class="flex shrink-0 items-center gap-1.5">
          <button
            type="button"
            disabled={atFirst}
            class="flex items-center gap-0.5 rounded-md border border-[var(--hairline)] py-1 pl-1 pr-2 text-[12px] text-base-content/80 transition-colors hover:bg-base-300/60 hover:text-base-content disabled:pointer-events-none disabled:opacity-35"
            onclick={() => goTo(index - 1)}
          >
            <ChevronLeft size={13} class="shrink-0" />
            {t("agent.question.prev")}
          </button>
          <button
            type="button"
            disabled={atLast}
            class="flex items-center gap-0.5 rounded-md border border-[var(--hairline)] py-1 pl-2 pr-1 text-[12px] text-base-content/80 transition-colors hover:bg-base-300/60 hover:text-base-content disabled:pointer-events-none disabled:opacity-35"
            onclick={() => goTo(index + 1)}
          >
            {t("agent.question.next")}
            <ChevronRight size={13} class="shrink-0" />
          </button>
        </div>
        <!-- Dots double as a jump target and as an at-a-glance answered map. -->
        <div class="flex shrink-0 items-center gap-1.5">
          {#each request.questions as question, i (question.id)}
            <button
              type="button"
              aria-label={t("agent.question.goTo", { index: i + 1 })}
              aria-current={i === index}
              class={`h-1.5 rounded-full transition-all ${
                i === index
                  ? "w-4 bg-primary"
                  : isAnswered(question)
                    ? "w-1.5 bg-primary/45 hover:bg-primary/70"
                    : "w-1.5 bg-base-content/25 hover:bg-base-content/45"
              }`}
              onclick={() => goTo(i)}
            ></button>
          {/each}
        </div>
      {/if}
    </div>
    <div class="flex shrink-0 items-center gap-2">
      <Button size="sm" variant="secondary" onclick={dismiss}
        >{t("agent.question.dismiss")}</Button
      >
      <Button
        size="sm"
        variant="primary"
        disabled={!canSubmit}
        title={canSubmit ? "" : t("agent.question.submitBlocked")}
        onclick={submit}>{t("agent.question.submit")}</Button
      >
    </div>
  </div>
</section>
