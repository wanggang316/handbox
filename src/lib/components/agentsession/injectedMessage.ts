/**
 * Telling the reader's own words apart from text a hook put in their mouth.
 *
 * Two kinds of user message reach the transcript without anybody typing them:
 * the context an extension contributes ahead of a prompt, and the reason a
 * `turn_end` rule returns to deny the stop — upstream resumes the loop with
 * that reason as the next user turn. Both arrive as plain `role: "user"`
 * messages, so rendered naively they are indistinguishable from something the
 * reader sent, and a second reply to a question they only asked once reads as
 * the app talking to itself.
 *
 * Both envelopes name their source, which is what makes them recognisable here.
 * The attribution is not decoration: an instruction the model reads as the
 * user's own carries authority nobody granted it (see `build_extension_context_message`
 * upstream, and `on_turn_end` in services/agent_hook_rules.rs).
 */

export type InjectedKind = "context" | "continuation";

export interface InjectedMessage {
  /** context = contributed ahead of a prompt; continuation = a denied turn end. */
  kind: InjectedKind;
  /** Extension name or hook-rule name, as written in the envelope. */
  source: string;
  /** The injected body, envelope stripped. */
  text: string;
}

/**
 * One envelope at the current offset. Sticky so a scan can only ever consume
 * consecutive blocks — a tag buried mid-message never matches, which is what
 * keeps hand-typed text carrying the same words from being mistaken for a hook.
 */
const ENVELOPES: { kind: InjectedKind; pattern: RegExp }[] = [
  {
    kind: "context",
    pattern: /<extension-context extension="([^"]*)">\n?([\s\S]*?)\n?<\/extension-context>/y,
  },
  {
    kind: "continuation",
    pattern: /<hook-continuation rule="([^"]*)">\n?([\s\S]*?)\n?<\/hook-continuation>/y,
  },
];

/** Whitespace between consecutive blocks (upstream joins them with a newline). */
const GAP = /\s*/y;

/**
 * Parse a sent user message into the hook blocks that make it up, or `null`
 * when it is the reader's own message.
 *
 * All-or-nothing by design: the whole message must consist of envelopes, since
 * the two producers each emit a message holding nothing else. A partial match
 * means the reader typed something that merely looks like a tag, and their text
 * is returned to the bubble untouched.
 */
export function parseInjectedMessage(raw: string): InjectedMessage[] | null {
  const source = raw.trim();
  if (!source.startsWith("<")) {
    return null;
  }

  const blocks: InjectedMessage[] = [];
  let offset = 0;
  while (offset < source.length) {
    const block = ENVELOPES.reduce<InjectedMessage | null>((found, envelope) => {
      if (found) {
        return found;
      }
      envelope.pattern.lastIndex = offset;
      const match = envelope.pattern.exec(source);
      if (!match) {
        return null;
      }
      offset = envelope.pattern.lastIndex;
      return { kind: envelope.kind, source: match[1], text: match[2].trim() };
    }, null);

    if (!block) {
      return null;
    }
    blocks.push(block);

    GAP.lastIndex = offset;
    GAP.exec(source);
    offset = GAP.lastIndex;
  }

  return blocks.length > 0 ? blocks : null;
}
