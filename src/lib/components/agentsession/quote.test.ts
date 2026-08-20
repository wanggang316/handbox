import { describe, expect, it } from "vitest";
import { withQuote } from "./quote";

describe("withQuote", () => {
  it("returns the text unchanged when nothing is quoted", () => {
    expect(withQuote("why?", null)).toBe("why?");
    expect(withQuote("why?", "")).toBe("why?");
  });

  it("prepends the quote as a blockquote, separated by a blank line", () => {
    expect(withQuote("why?", "OSGi")).toBe("> OSGi\n\nwhy?");
  });

  it("quotes every line of a multi-line selection", () => {
    expect(withQuote("why?", "first\nsecond")).toBe(
      "> first\n> second\n\nwhy?",
    );
  });

  it("keeps blank lines inside the quote as part of the block", () => {
    expect(withQuote("why?", "first\n\nsecond")).toBe(
      "> first\n>\n> second\n\nwhy?",
    );
  });

  it("sends the quote alone when the composer is empty", () => {
    expect(withQuote("", "OSGi")).toBe("> OSGi");
    expect(withQuote("   ", "OSGi")).toBe("> OSGi");
  });
});
