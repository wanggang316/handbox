<script lang="ts">
  /**
   * Floating "quote this" affordance for transcript text selections.
   *
   * Watches the document selection and, whenever it lands inside `container`,
   * parks a small pill next to it; pressing it hands the selected text to the
   * composer. The text is captured when the pill appears, so the action never
   * depends on the selection surviving the press.
   */
  import { Reply } from "@lucide/svelte";
  import { t } from "$lib/i18n";

  interface Props {
    /**
     * Selections outside this element are ignored (the transcript content).
     * Anything under a `data-no-quote` ancestor is excluded as well.
     */
    container: HTMLElement | undefined;
    /**
     * Visible region the pill must stay inside — the transcript scroller. The
     * window's top 50px is a Tauri drag region (see TitleBar): a pill parked
     * under it is dead, because macOS turns the press into a window drag, and a
     * window drag neither clicks nor collapses the selection that keeps it up.
     */
    viewport: HTMLElement | undefined;
    onReply: (text: string) => void;
  }

  let { container, viewport, onReply }: Props = $props();

  // Gap between the selection and the pill. Its height is fixed (`h-7`) rather
  // than measured: placement has to be decided before the pill is in the DOM.
  const GAP = 8;
  const PILL_HEIGHT = 28;
  // Right-edge clamp uses a fixed estimate: the pill is a two-element label of
  // known size, and measuring it would require a render pass per placement.
  const PILL_WIDTH = 120;

  let quoted = $state("");
  let placement = $state<{ top: number; left: number } | null>(null);
  let buttonEl = $state<HTMLButtonElement>();

  function hide() {
    placement = null;
    quoted = "";
  }

  // The live selection, or null unless it is a non-empty range inside
  // `container` and outside every `data-no-quote` region.
  function readSelection(): { text: string; rect: DOMRect } | null {
    const el = container;
    if (!el) {
      return null;
    }
    const selection = window.getSelection();
    if (!selection || selection.isCollapsed || selection.rangeCount === 0) {
      return null;
    }
    const text = selection.toString().trim();
    if (!text) {
      return null;
    }
    const range = selection.getRangeAt(0);
    const node = range.commonAncestorContainer;
    const anchor =
      node.nodeType === Node.ELEMENT_NODE
        ? (node as Element)
        : node.parentElement;
    if (!anchor || !el.contains(anchor) || anchor.closest("[data-no-quote]")) {
      return null;
    }
    // First client rect, not the bounding box: a multi-line selection anchors
    // the pill at where the reader started selecting, not at the block's corner.
    const rect = range.getClientRects()[0] ?? range.getBoundingClientRect();
    return { text, rect };
  }

  // Above the selection when it fits inside the viewport, below it otherwise,
  // then clamped so the pill can never leave the viewport on any edge.
  function place(rect: DOMRect): { top: number; left: number } {
    const bounds = viewport?.getBoundingClientRect();
    const minTop = (bounds?.top ?? 0) + GAP;
    const maxTop = (bounds?.bottom ?? window.innerHeight) - PILL_HEIGHT - GAP;
    const above = rect.top - GAP - PILL_HEIGHT;
    const top = above >= minTop ? above : rect.bottom + GAP;
    return {
      top: Math.min(Math.max(top, minTop), Math.max(minTop, maxTop)),
      left: Math.min(
        Math.max(GAP, rect.left),
        Math.max(GAP, window.innerWidth - PILL_WIDTH - GAP),
      ),
    };
  }

  function syncFromSelection() {
    const found = readSelection();
    if (!found) {
      hide();
      return;
    }
    quoted = found.text;
    placement = place(found.rect);
  }

  function takeQuote() {
    const text = quoted;
    hide();
    window.getSelection()?.removeAllRanges();
    if (text) {
      onReply(text);
    }
  }

  // Pointer path: acting on pointerdown (rather than click) keeps the quote one
  // press away even where a later click would be swallowed, and the
  // preventDefault suppresses the mouse events that would move focus and
  // collapse the selection. The click handler is the keyboard path — the two
  // never both fire, since the pill unmounts on the first of them.
  function handlePointerDown(event: PointerEvent) {
    event.preventDefault();
    takeQuote();
  }

  $effect(() => {
    if (!container) {
      return;
    }

    // The selection is only final after the browser has processed mouseup, so
    // read it a frame later; shift-selection is caught on keyup instead.
    let frame = 0;
    const onMouseUp = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(syncFromSelection);
    };
    const onKeyUp = (event: KeyboardEvent) => {
      if (event.shiftKey || event.key === "Shift") {
        syncFromSelection();
      }
    };
    // Selection changes only ever take the pill away: showing it mid-drag would
    // have it chase the pointer. This is also the backstop that keeps it from
    // getting stuck — whatever collapses the selection also dismisses the pill.
    const onSelectionChange = () => {
      if (placement && !readSelection()) {
        hide();
      }
    };
    // A new press starts a new interaction, so the pill goes away — except when
    // the press is the one consuming it.
    const onMouseDown = (event: MouseEvent) => {
      const target = event.target;
      if (target instanceof Node && buttonEl?.contains(target)) {
        return;
      }
      hide();
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        hide();
      }
    };
    // Capture: the transcript scroller is the usual scroll source, and fixed
    // coordinates go stale the moment it moves.
    const onScroll = () => hide();

    document.addEventListener("mouseup", onMouseUp);
    document.addEventListener("mousedown", onMouseDown);
    document.addEventListener("keyup", onKeyUp);
    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("selectionchange", onSelectionChange);
    document.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onScroll);

    return () => {
      cancelAnimationFrame(frame);
      document.removeEventListener("mouseup", onMouseUp);
      document.removeEventListener("mousedown", onMouseDown);
      document.removeEventListener("keyup", onKeyUp);
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("selectionchange", onSelectionChange);
      document.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onScroll);
    };
  });
</script>

{#if placement}
  <!-- Fixed to the viewport: the selection lives in a scrolling column that
       would clip an absolutely positioned pill. -->
  <button
    bind:this={buttonEl}
    type="button"
    style={`top: ${placement.top}px; left: ${placement.left}px;`}
    class="fixed z-[var(--z-popover)] flex h-7 items-center gap-1.5 rounded-md border border-[var(--hairline)] bg-[var(--bg-card)] px-2.5 text-xs text-base-content/80 shadow-lg transition-colors hover:bg-base-300"
    aria-label={t("agent.timeline.quoteReply")}
    onpointerdown={handlePointerDown}
    onclick={takeQuote}
  >
    <Reply size={13} class="shrink-0" />
    <span>{t("agent.timeline.quoteReply")}</span>
  </button>
{/if}
