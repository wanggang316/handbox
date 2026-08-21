/**
 * Composing a quoted reply: the reader selects a passage in the transcript and
 * the composer sends it in front of their own message.
 *
 * The passage travels inside an XML-style envelope rather than as a markdown
 * blockquote. Both are legible to a model, but a tag pair is an unambiguous
 * boundary: quoted material can only ever come from agent output (tool results,
 * fetched pages, generated text), and text that merely starts with "> " gives
 * the model nothing to distinguish "this is what I am asking about" from "this
 * is what I am telling you to do". It also survives verbatim — line prefixes
 * would rewrite indented code and tables inside the quote.
 *
 * The envelope is a wire format, not something the reader should ever see: the
 * transcript splits it back apart with `splitQuote` and renders the quote as a
 * quote.
 */

const TAG = "quoted_text";

/** Leading envelope plus the blank line separating it from the message. */
const ENVELOPE = /^<quoted_text>\n?([\s\S]*?)\n?<\/quoted_text>\n*/;

/** A closing tag inside the passage would end the envelope early. */
const CLOSING_TAG = /<\/(quoted_text)\s*>/gi;
const ESCAPED_CLOSING_TAG = /&lt;\/(quoted_text)>/gi;

/**
 * Wrap `quoted` in the envelope and put it in front of `text`, so the model
 * reads the passage the message is about before reading the message itself.
 * Empty composer text sends the quote alone.
 */
export function withQuote(text: string, quoted: string | null): string {
  if (!quoted) {
    return text;
  }
  const body = quoted.replace(CLOSING_TAG, "&lt;/$1>").trim();
  const block = `<${TAG}>\n${body}\n</${TAG}>`;
  return text.trim() ? `${block}\n\n${text}` : block;
}

export interface QuotedMessage {
  /** The quoted passage, or null when the message carries no envelope. */
  quote: string | null;
  /** What the user typed themselves; empty when they sent the quote alone. */
  text: string;
}

/**
 * Split a sent user message back into its quote and the message proper. A
 * message without a leading envelope is returned unchanged, so anything typed
 * by hand — including text that happens to mention the tag — is never mangled.
 */
export function splitQuote(raw: string): QuotedMessage {
  const match = raw.match(ENVELOPE);
  if (!match) {
    return { quote: null, text: raw };
  }
  return {
    quote: match[1].replace(ESCAPED_CLOSING_TAG, "</$1>").trim(),
    text: raw.slice(match[0].length),
  };
}
