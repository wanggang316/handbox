import { describe, expect, it } from "vitest";
import { parseInjectedMessage } from "./injectedMessage";

describe("parseInjectedMessage", () => {
  it("returns null for a message the reader typed", () => {
    expect(parseInjectedMessage("测试一下，介绍一下北京，不少于 200 字")).toBeNull();
    expect(parseInjectedMessage("")).toBeNull();
  });

  it("reads the context an extension contributed", () => {
    expect(
      parseInjectedMessage(
        '<extension-context extension="handbox-hook-rules">\nCONTEXT-FROM-HOOK: current HandBox version is 0.4.4\n</extension-context>',
      ),
    ).toEqual([
      {
        kind: "context",
        source: "handbox-hook-rules",
        text: "CONTEXT-FROM-HOOK: current HandBox version is 0.4.4",
      },
    ]);
  });

  it("reads the reason a denied turn end resumed on", () => {
    expect(
      parseInjectedMessage(
        '<hook-continuation rule="enforce tests">\nRun the tests you said you would.\n</hook-continuation>',
      ),
    ).toEqual([
      {
        kind: "continuation",
        source: "enforce tests",
        text: "Run the tests you said you would.",
      },
    ]);
  });

  it("reads every block when several extensions contribute at once", () => {
    const blocks = parseInjectedMessage(
      '<extension-context extension="a">\nfirst\n</extension-context>\n<extension-context extension="b">\nsecond\n</extension-context>',
    );
    expect(blocks).toHaveLength(2);
    expect(blocks?.map((block) => block.source)).toEqual(["a", "b"]);
  });

  it("tolerates surrounding whitespace", () => {
    expect(
      parseInjectedMessage(
        '\n  <extension-context extension="a">\nbody\n</extension-context>  \n',
      ),
    ).toEqual([{ kind: "context", source: "a", text: "body" }]);
  });

  it("keeps a hand-typed message that merely mentions a tag", () => {
    // Partial matches are the reader's words: only a message made entirely of
    // envelopes came from a hook.
    expect(
      parseInjectedMessage(
        'what does <extension-context extension="a">\nx\n</extension-context> mean?',
      ),
    ).toBeNull();
    expect(
      parseInjectedMessage(
        '<extension-context extension="a">\nx\n</extension-context> and then some',
      ),
    ).toBeNull();
    expect(
      parseInjectedMessage('<extension-context extension="a">unclosed'),
    ).toBeNull();
  });

  it("keeps an empty body rather than dropping the block", () => {
    expect(
      parseInjectedMessage('<extension-context extension="a"></extension-context>'),
    ).toEqual([{ kind: "context", source: "a", text: "" }]);
  });
});
