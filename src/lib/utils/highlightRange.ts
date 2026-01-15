import type { TextRange } from "$lib/types/favorite";

const HIGHLIGHT_CLASS = "favorite-highlight bg-amber-500/20 px-1 rounded";
const HIGHLIGHT_ATTR = "data-favorite-highlight";
const HIGHLIGHT_INDEX_ATTR = "data-favorite-range-index";

type HighlightRangeOptions =
  | TextRange[]
  | TextRange
  | null
  | undefined
  | {
      ranges?: TextRange[] | TextRange | null;
      onRangeHover?: (payload: { range: TextRange; rect: DOMRect }) => void;
      onRangeLeave?: () => void;
      hoverDelayMs?: number;
      version?: number;
    };

function isTextRange(value: unknown): value is TextRange {
  return Boolean(value) && typeof value === "object" && "start" in value && "end" in value;
}

function normalizeRanges(input: HighlightRangeOptions): TextRange[] {
  const list = toRangeList(input);
  return list
    .map((range) => ({
      start: Math.max(0, Math.floor(range.start)),
      end: Math.max(0, Math.floor(range.end)),
    }))
    .filter((range) => range.end > range.start)
    .sort((a, b) => a.start - b.start);
}

function toRangeList(input: HighlightRangeOptions): TextRange[] {
  if (!input) return [];
  if (typeof input === "object" && "ranges" in input) {
    return toRangeList(input.ranges ?? []);
  }
  if (Array.isArray(input)) return input;
  if (typeof input === "object" && "length" in input) {
    return Array.from(input as ArrayLike<TextRange>);
  }
  if (isTextRange(input)) return [input];
  if (typeof input === "object") {
    const values = Object.values(input).filter(isTextRange);
    if (values.length > 0) return values;
  }
  return [];
}

function mergeRanges(ranges: TextRange[]): TextRange[] {
  if (ranges.length <= 1) return ranges;
  const merged: TextRange[] = [];
  let current = ranges[0];

  for (let i = 1; i < ranges.length; i += 1) {
    const next = ranges[i];
    if (next.start <= current.end) {
      current = { start: current.start, end: Math.max(current.end, next.end) };
    } else {
      merged.push(current);
      current = next;
    }
  }

  merged.push(current);
  return merged;
}

function clearHighlights(node: HTMLElement): void {
  const highlights = node.querySelectorAll<HTMLElement>(`[${HIGHLIGHT_ATTR}]`);
  highlights.forEach((highlight) => {
    const parent = highlight.parentNode;
    if (!parent) return;
    const text = document.createTextNode(highlight.textContent ?? "");
    parent.replaceChild(text, highlight);
    parent.normalize();
  });
}

function collectTextNodes(root: HTMLElement): Text[] {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const nodes: Text[] = [];
  let current = walker.nextNode();
  while (current) {
    nodes.push(current as Text);
    current = walker.nextNode();
  }
  return nodes;
}

function buildFragment(
  text: string,
  ranges: Array<{ start: number; end: number; rangeIndex: number }>,
): DocumentFragment {
  const fragment = document.createDocumentFragment();
  let cursor = 0;

  for (const range of ranges) {
    if (range.start > cursor) {
      fragment.append(document.createTextNode(text.slice(cursor, range.start)));
    }

    const selectedText = text.slice(range.start, range.end);
    if (!selectedText.trim()) {
      fragment.append(document.createTextNode(selectedText));
    } else {
      const highlight = document.createElement("span");
      highlight.setAttribute(HIGHLIGHT_ATTR, "true");
      highlight.setAttribute(HIGHLIGHT_INDEX_ATTR, String(range.rangeIndex));
      highlight.className = HIGHLIGHT_CLASS;
      highlight.textContent = selectedText;
      fragment.append(highlight);
    }
    cursor = range.end;
  }

  if (cursor < text.length) {
    fragment.append(document.createTextNode(text.slice(cursor)));
  }

  return fragment;
}

function applyHighlights(node: HTMLElement, ranges: TextRange[]): number {
  clearHighlights(node);
  let highlightCount = 0;
  const textNodes = collectTextNodes(node);
  let globalIndex = 0;
  let rangeIndex = 0;
  let currentRange = ranges[rangeIndex];

  for (const textNode of textNodes) {
    const text = textNode.textContent ?? "";
    const length = text.length;
    if (length === 0) {
      continue;
    }

    const nodeStart = globalIndex;
    const nodeEnd = globalIndex + length;

    while (currentRange && currentRange.end <= nodeStart) {
      rangeIndex += 1;
      currentRange = ranges[rangeIndex];
    }

    if (!currentRange) {
      break;
    }

    if (currentRange.start >= nodeEnd) {
      globalIndex = nodeEnd;
      continue;
    }

    const segments: Array<{ start: number; end: number; rangeIndex: number }> = [];
    while (currentRange && currentRange.start < nodeEnd) {
      const start = Math.max(currentRange.start, nodeStart) - nodeStart;
      const end = Math.min(currentRange.end, nodeEnd) - nodeStart;
      if (end > start) {
        segments.push({ start, end, rangeIndex });
        const selectedText = text.slice(start, end);
        if (selectedText.trim()) {
          highlightCount += 1;
        }
      }

      if (currentRange.end <= nodeEnd) {
        rangeIndex += 1;
        currentRange = ranges[rangeIndex];
      } else {
        break;
      }
    }

    if (segments.length > 0) {
      const fragment = buildFragment(text, segments);
      const parent = textNode.parentNode;
      if (parent) {
        parent.replaceChild(fragment, textNode);
      }
    }

    globalIndex = nodeEnd;
  }

  return highlightCount;
}

function getOffsetInContainer(container: HTMLElement, node: Node, offset: number): number {
  const range = document.createRange();
  range.selectNodeContents(container);
  range.setEnd(node, offset);
  const fragment = range.cloneContents();
  return fragment.textContent?.length ?? 0;
}

export function getSelectionTextRange(
  container: HTMLElement,
  selection: Selection | null,
): TextRange | null {
  if (!selection || selection.rangeCount === 0) return null;
  const range = selection.getRangeAt(0);

  if (
    !container.contains(range.startContainer) ||
    !container.contains(range.endContainer)
  ) {
    return null;
  }

  const start = getOffsetInContainer(container, range.startContainer, range.startOffset);
  const end = getOffsetInContainer(container, range.endContainer, range.endOffset);

  if (start === end) return null;
  return { start: Math.min(start, end), end: Math.max(start, end) };
}

export function highlightRange(node: HTMLElement, input: HighlightRangeOptions) {
  let options: HighlightRangeOptions = input;
  let hoverTimer: number | null = null;
  let lastTarget: HTMLElement | null = null;
  let lastSignature = "";
  let lastTextLength = -1;
  let lastRangeCount = 0;
  let lastVersion: number | undefined;

  const handleMouseOver = (event: MouseEvent) => {
    if (!options || typeof options !== "object" || !("onRangeHover" in options)) {
      return;
    }
    const target = (event.target as HTMLElement | null)?.closest<HTMLElement>(
      `[${HIGHLIGHT_ATTR}]`,
    );
    if (!target || !node.contains(target)) return;
    if (target === lastTarget) return;
    lastTarget = target;

    const ranges = normalizeRanges(options);
    const index = Number(target.getAttribute(HIGHLIGHT_INDEX_ATTR));
    if (!Number.isFinite(index) || !ranges[index]) return;

    if (hoverTimer) {
      window.clearTimeout(hoverTimer);
    }
    hoverTimer = window.setTimeout(() => {
      const rect = target.getBoundingClientRect();
      options.onRangeHover?.({ range: ranges[index], rect });
    }, options.hoverDelayMs ?? 2000);
  };

  const handleMouseOut = (event: MouseEvent) => {
    const target = (event.target as HTMLElement | null)?.closest<HTMLElement>(
      `[${HIGHLIGHT_ATTR}]`,
    );
    if (!target || !node.contains(target)) return;
    if (hoverTimer) {
      window.clearTimeout(hoverTimer);
      hoverTimer = null;
    }
    lastTarget = null;
    if (options && typeof options === "object" && "onRangeLeave" in options) {
      options.onRangeLeave?.();
    }
  };

  node.addEventListener("mouseover", handleMouseOver);
  node.addEventListener("mouseout", handleMouseOut);

  const run = (nextInput: HighlightRangeOptions) => {
    options = nextInput;
    const normalized = mergeRanges(normalizeRanges(options));
    const signature = normalized.map((range) => `${range.start}-${range.end}`).join(",");
    const textLength = node.textContent?.length ?? 0;
    const version =
      typeof options === "object" && options && "version" in options
        ? options.version
        : undefined;

    if (
      signature === lastSignature &&
      textLength === lastTextLength &&
      version === lastVersion
    ) {
      return;
    }

    lastSignature = signature;
    lastTextLength = textLength;
    lastVersion = version;

    node.dataset.favoriteRangeCount = String(normalized.length);
    if (normalized.length === 0) {
      if (lastRangeCount > 0) {
        clearHighlights(node);
      }
      node.dataset.favoriteHighlightCount = "0";
      lastRangeCount = 0;
      return;
    }

    const highlightCount = applyHighlights(node, normalized);
    node.dataset.favoriteHighlightCount = String(highlightCount);
    lastRangeCount = normalized.length;
  };

  run(input);

  return {
    update(nextInput: HighlightRangeOptions) {
      run(nextInput);
    },
    destroy() {
      clearHighlights(node);
      node.removeEventListener("mouseover", handleMouseOver);
      node.removeEventListener("mouseout", handleMouseOut);
    },
  };
}
