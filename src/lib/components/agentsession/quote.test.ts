import { describe, expect, it } from "vitest";
import { splitQuote, withQuote } from "./quote";

describe("withQuote", () => {
  it("returns the text unchanged when nothing is quoted", () => {
    expect(withQuote("why?", null)).toBe("why?");
    expect(withQuote("why?", "")).toBe("why?");
  });

  it("wraps the quote in an envelope ahead of the message", () => {
    expect(withQuote("why?", "OSGi")).toBe(
      "<quoted_text>\nOSGi\n</quoted_text>\n\nwhy?",
    );
  });

  it("keeps a multi-line passage verbatim", () => {
    expect(withQuote("why?", "  indented\n\nsecond")).toBe(
      "<quoted_text>\nindented\n\nsecond\n</quoted_text>\n\nwhy?",
    );
  });

  it("escapes a closing tag that would end the envelope early", () => {
    expect(withQuote("why?", "a </quoted_text> b")).toBe(
      "<quoted_text>\na &lt;/quoted_text> b\n</quoted_text>\n\nwhy?",
    );
  });

  it("sends the quote alone when the composer is empty", () => {
    expect(withQuote("   ", "OSGi")).toBe("<quoted_text>\nOSGi\n</quoted_text>");
  });
});

describe("splitQuote", () => {
  it("leaves a message without an envelope alone", () => {
    expect(splitQuote("why?")).toEqual({ quote: null, text: "why?" });
  });

  it("splits an envelope from the message that follows it", () => {
    expect(splitQuote("<quoted_text>\nOSGi\n</quoted_text>\n\nwhy?")).toEqual({
      quote: "OSGi",
      text: "why?",
    });
  });

  it("reads a quote sent on its own", () => {
    expect(splitQuote("<quoted_text>\nOSGi\n</quoted_text>")).toEqual({
      quote: "OSGi",
      text: "",
    });
  });

  it("restores an escaped closing tag", () => {
    expect(
      splitQuote("<quoted_text>\na &lt;/quoted_text> b\n</quoted_text>\n\nwhy?")
        .quote,
    ).toBe("a </quoted_text> b");
  });

  it("only reads an envelope at the start of the message", () => {
    const raw = "why? <quoted_text>\nOSGi\n</quoted_text>";
    expect(splitQuote(raw)).toEqual({ quote: null, text: raw });
  });

  it("round-trips what withQuote composed", () => {
    expect(splitQuote(withQuote("why?", "first\nsecond"))).toEqual({
      quote: "first\nsecond",
      text: "why?",
    });
  });
});
