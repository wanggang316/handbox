<script lang="ts">
  /**
   * Floating "quote this" affordance for transcript text selections.
   *
   * Watches the document selection and, whenever it lands inside `container`,
   * parks a small pill next to it; clicking hands the selected text to the
   * composer. The text is captured when the pill appears, so a click never
   * depends on the selection surviving the interaction.
   */
  import { Reply } from "@lucide/svelte";
  import { fly } from "svelte/transition";
  import { t } from "$lib/i18n";

  interface Props {
    /** Selections outside this element are ignored (the transcript column). */
    container: HTMLElement | undefined;
    onReply: (text: string) => void;
  }

  let { container, onReply }: Props = $props();

  // Distance between the selection and the pill, and the headroom the pill
  // needs above the selection before it flips below it.
  const GAP = 8;
  const FLIP_MARGIN = 44;
  // Right-edge clamp uses a fixed estimate: the pill is a two-element label of
  // known size, and measuring it would require a render pass per placement.
  const PILL_WIDTH = 120;

  let quoted = $state("");
  let placement = $state<{ top: number; left: number; above: boolean } | null>(
    null,
  );
  let buttonEl = $state<HTMLButtonElement>();

  function hide() {
    placement = null;
    quoted = "";
  }

  // The live selection, or null unless it is a non-empty range inside `container`.
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
    if (!anchor || !el.contains(anchor)) {
      return null;
    }
    // First client rect, not the bounding box: a multi-line selection anchors
    // the pill at where the reader started selecting, not at the block's corner.
    const rect = range.getClientRects()[0] ?? range.getBoundingClientRect();
    return { text, rect };
  }

  function syncFromSelection() {
    const found = readSelection();
    if (!found) {
      hide();
      return;
    }
    const { text, rect } = found;
    const above = rect.top > FLIP_MARGIN;
    quoted = text;
    placement = {
      top: above ? rect.top - GAP : rect.bottom + GAP,
      left: Math.min(
        Math.max(GAP, rect.left),
        Math.max(GAP, window.innerWidth - PILL_WIDTH - GAP),
      ),
      above,
    };
  }

  function handleClick() {
    const text = quoted;
    hide();
    window.getSelection()?.removeAllRanges();
    if (text) {
      onReply(text);
    }
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
    // A new press starts a new interaction, so the pill goes away — except when
    // the press is the click that consumes it.
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
    document.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onScroll);

    return () => {
      cancelAnimationFrame(frame);
      document.removeEventListener("mouseup", onMouseUp);
      document.removeEventListener("mousedown", onMouseDown);
      document.removeEventListener("keyup", onKeyUp);
      document.removeEventListener("keydown", onKeyDown);
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
    class={`fixed z-[var(--z-popover)] flex items-center gap-1.5 rounded-md border border-[var(--hairline)] bg-[var(--bg-card)] px-2.5 py-1.5 text-xs text-base-content/80 shadow-lg transition-colors hover:bg-base-300 ${
      placement.above ? "-translate-y-full" : ""
    }`}
    aria-label={t("agent.timeline.quoteReply")}
    onmousedown={(event) => event.preventDefault()}
    onclick={handleClick}
    transition:fly={{ y: placement.above ? 4 : -4, duration: 120 }}
  >
    <Reply size={13} class="shrink-0" />
    <span>{t("agent.timeline.quoteReply")}</span>
  </button>
{/if}
