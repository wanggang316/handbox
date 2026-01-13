import type { TextRange } from "$lib/types/favorite";

const HIGHLIGHT_CLASS = "favorite-highlight bg-amber-500/20 px-1 rounded";
const HIGHLIGHT_ATTR = "data-favorite-highlight";

function normalizeRanges(
  ranges: TextRange[] | TextRange | null | undefined,
): TextRange[] {
  const list = toRangeList(ranges);
  return list
    .map((range) => ({
      start: Math.max(0, Math.floor(range.start)),
      end: Math.max(0, Math.floor(range.end)),
    }))
    .filter((range) => range.end > range.start)
    .sort((a, b) => a.start - b.start);
}

function toRangeList(
  ranges: TextRange[] | TextRange | null | undefined,
): TextRange[] {
  if (!ranges) return [];
  if (Array.isArray(ranges)) return ranges;
  if (typeof ranges === "object" && "length" in ranges) {
    return Array.from(ranges as ArrayLike<TextRange>);
  }
  return [ranges];
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

function buildFragment(text: string, ranges: Array<{ start: number; end: number }>): DocumentFragment {
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

function applyHighlights(
  node: HTMLElement,
  ranges: TextRange[] | TextRange | null | undefined,
): void {
  clearHighlights(node);
  const normalized = mergeRanges(normalizeRanges(ranges));
  node.dataset.favoriteHighlightCount = String(normalized.length);
  if (normalized.length === 0) return;

  if (import.meta.env.DEV) {
    const textLength = node.textContent?.length ?? 0;
    const maxEnd = Math.max(...normalized.map((range) => range.end));
    if (maxEnd > textLength) {
      console.debug("[highlightRange] range exceeds text length", {
        textLength,
        maxEnd,
      });
    }
  }

  const textNodes = collectTextNodes(node);
  let globalIndex = 0;
  let rangeIndex = 0;
  let currentRange = normalized[rangeIndex];

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
      currentRange = normalized[rangeIndex];
    }

    if (!currentRange) {
      break;
    }

    if (currentRange.start >= nodeEnd) {
      globalIndex = nodeEnd;
      continue;
    }

    const segments: Array<{ start: number; end: number }> = [];
    while (currentRange && currentRange.start < nodeEnd) {
      const start = Math.max(currentRange.start, nodeStart) - nodeStart;
      const end = Math.min(currentRange.end, nodeEnd) - nodeStart;
      if (end > start) {
        segments.push({ start, end });
      }

      if (currentRange.end <= nodeEnd) {
        rangeIndex += 1;
        currentRange = normalized[rangeIndex];
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
}

function getOffsetInContainer(container: HTMLElement, node: Node, offset: number): number {
  const range = document.createRange();
  range.selectNodeContents(container);
  range.setEnd(node, offset);
  return range.toString().length;
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

export function highlightRange(
  node: HTMLElement,
  ranges: TextRange[] | TextRange | null | undefined,
) {
  const run = (nextRanges: TextRange[] | TextRange | null | undefined) => {
    applyHighlights(node, nextRanges);
  };

  run(ranges);

  return {
    update(nextRanges: TextRange[] | TextRange | null | undefined) {
      run(nextRanges);
    },
    destroy() {
      clearHighlights(node);
      delete node.dataset.favoriteHighlightCount;
    },
  };
}
