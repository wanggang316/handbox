import { describe, expect, it } from "vitest";
import { parseRenderCardArgs } from "./renderCard";

describe("parseRenderCardArgs", () => {
  it("accepts an object with html", () => {
    expect(parseRenderCardArgs({ html: "<div>x</div>" })).toEqual({
      html: "<div>x</div>",
      title: undefined,
    });
  });

  it("carries a non-empty title through", () => {
    expect(parseRenderCardArgs({ html: "<p>x</p>", title: "Demo" })).toEqual({
      html: "<p>x</p>",
      title: "Demo",
    });
  });

  it("drops an empty/whitespace/non-string title", () => {
    expect(parseRenderCardArgs({ html: "<p>x</p>", title: "  " })?.title).toBe(
      undefined,
    );
    expect(parseRenderCardArgs({ html: "<p>x</p>", title: 42 })?.title).toBe(
      undefined,
    );
  });

  it("accepts a JSON-string carrier", () => {
    expect(parseRenderCardArgs('{"html":"<b>ok</b>","title":"T"}')).toEqual({
      html: "<b>ok</b>",
      title: "T",
    });
  });

  it("rejects malformed JSON strings (streaming partials)", () => {
    expect(parseRenderCardArgs('{"html":"<div')).toBe(null);
  });

  it("rejects missing/empty/non-string html", () => {
    expect(parseRenderCardArgs({})).toBe(null);
    expect(parseRenderCardArgs({ html: "" })).toBe(null);
    expect(parseRenderCardArgs({ html: "   " })).toBe(null);
    expect(parseRenderCardArgs({ html: 42 })).toBe(null);
  });

  it("rejects non-object carriers", () => {
    expect(parseRenderCardArgs(null)).toBe(null);
    expect(parseRenderCardArgs(undefined)).toBe(null);
    expect(parseRenderCardArgs(["<div>"])).toBe(null);
    expect(parseRenderCardArgs(7)).toBe(null);
  });
});
