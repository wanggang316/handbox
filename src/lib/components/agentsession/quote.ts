/**
 * Composing a quoted reply: the reader selects a passage in the transcript and
 * the composer sends it in front of their own message.
 */

/**
 * Prepend `quoted` to `text` as a markdown blockquote, so the model reads the
 * passage the message refers to before reading the message itself. Blank lines
 * keep the quote one contiguous block; empty composer text sends the quote alone.
 */
export function withQuote(text: string, quoted: string | null): string {
  if (!quoted) {
    return text;
  }
  const block = quoted
    .split("\n")
    .map((line) => (line.trim() ? `> ${line}` : ">"))
    .join("\n");
  return text.trim() ? `${block}\n\n${text}` : block;
}
