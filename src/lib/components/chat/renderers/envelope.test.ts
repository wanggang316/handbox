import { describe, expect, it } from "vitest";
import { parseEnvelope } from "./envelope";

describe("parseEnvelope", () => {
  // ---------------------------------------------------------------------------
  // Group 1 — bare JSON carrier (VAL-PARSE-001)
  // ---------------------------------------------------------------------------
  describe("bare JSON", () => {
    it("parses a bare translation envelope into { type, data }", () => {
      const content = '{"__render":"translation","data":{"text":"hello"}}';
      expect(parseEnvelope(content)).toEqual({
        type: "translation",
        data: { text: "hello" },
      });
    });

    it("tolerates leading and trailing whitespace around the object", () => {
      const content = '  \n\t {"__render":"translation","data":{"text":"hi"}}\n  ';
      expect(parseEnvelope(content)).toEqual({
        type: "translation",
        data: { text: "hi" },
      });
    });

    it("ignores extra top-level keys such as version/id", () => {
      const content =
        '{"version":1,"id":"abc","__render":"translation","data":{"text":"x"}}';
      expect(parseEnvelope(content)).toEqual({
        type: "translation",
        data: { text: "x" },
      });
    });

    it("returns null when non-whitespace text precedes the object", () => {
      const content = '说明：{"__render":"translation","data":{}}';
      expect(parseEnvelope(content)).toBeNull();
    });

    it("returns null when non-whitespace text follows the object", () => {
      const content = '{"__render":"translation","data":{}}\n说明';
      expect(parseEnvelope(content)).toBeNull();
    });

    it("does not loosely slice from first { to last }", () => {
      // A prefix plus an object must NOT be salvaged by greedy extraction.
      const content = 'here is the result: {"__render":"translation"}';
      expect(parseEnvelope(content)).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // Group 2 — fenced code block carrier (VAL-PARSE-002)
  // ---------------------------------------------------------------------------
  describe("```json fenced code block", () => {
    it("strips a simple json fence and parses the inner object", () => {
      const content = '```json\n{"__render":"translation","data":{"a":1}}\n```';
      expect(parseEnvelope(content)).toEqual({
        type: "translation",
        data: { a: 1 },
      });
    });

    it("accepts an uppercase JSON language token", () => {
      const content = '```JSON\n{"__render":"card","data":{}}\n```';
      expect(parseEnvelope(content)).toEqual({ type: "card", data: {} });
    });

    it("accepts an indented fence", () => {
      const content = '   ```json\n   {"__render":"card","data":{}}\n   ```';
      expect(parseEnvelope(content)).toEqual({ type: "card", data: {} });
    });

    it("accepts CRLF line endings", () => {
      const content = '```json\r\n{"__render":"card","data":{}}\r\n```';
      expect(parseEnvelope(content)).toEqual({ type: "card", data: {} });
    });

    it("accepts blank lines between fence and content", () => {
      const content = '```json\n\n{"__render":"card","data":{}}\n\n```';
      expect(parseEnvelope(content)).toEqual({ type: "card", data: {} });
    });

    it("trims whitespace around the inner JSON", () => {
      const content = '```json\n   {"__render":"card","data":{}}   \n```';
      expect(parseEnvelope(content)).toEqual({ type: "card", data: {} });
    });

    it("accepts a trailing info string after the json token", () => {
      const content = '```json title=x\n{"__render":"card","data":{}}\n```';
      expect(parseEnvelope(content)).toEqual({ type: "card", data: {} });
    });

    it("returns null for a fence with no language", () => {
      const content = '```\n{"__render":"card","data":{}}\n```';
      expect(parseEnvelope(content)).toBeNull();
    });

    it("returns null for a non-json language fence", () => {
      const content = '```ts\n{"__render":"card","data":{}}\n```';
      expect(parseEnvelope(content)).toBeNull();
    });

    it("returns null for a json5 language token", () => {
      const content = '```json5\n{"__render":"card","data":{}}\n```';
      expect(parseEnvelope(content)).toBeNull();
    });

    it("returns null for a jsonc language token", () => {
      const content = '```jsonc\n{"__render":"card","data":{}}\n```';
      expect(parseEnvelope(content)).toBeNull();
    });

    it("returns null when there are multiple code blocks", () => {
      const content =
        '```json\n{"__render":"card","data":{}}\n```\n```json\n{"__render":"other"}\n```';
      expect(parseEnvelope(content)).toBeNull();
    });

    it("returns null when prose follows the closing fence", () => {
      const content = '```json\n{"__render":"card","data":{}}\n```\n请查收';
      expect(parseEnvelope(content)).toBeNull();
    });

    it("returns null when prose precedes the opening fence", () => {
      const content = '说明：\n```json\n{"__render":"card","data":{}}\n```';
      expect(parseEnvelope(content)).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // Group 3 — structural rejection (VAL-PARSE-003)
  // ---------------------------------------------------------------------------
  describe("structural rejection", () => {
    it("returns null for a top-level array", () => {
      expect(parseEnvelope('[{"__render":"x"}]')).toBeNull();
    });

    it("returns null for a top-level string literal", () => {
      expect(parseEnvelope('"__render"')).toBeNull();
    });

    it("returns null for a top-level number literal", () => {
      expect(parseEnvelope("42")).toBeNull();
    });

    it("returns null for a top-level boolean literal", () => {
      expect(parseEnvelope("true")).toBeNull();
    });

    it("returns null for a top-level null literal", () => {
      expect(parseEnvelope("null")).toBeNull();
    });

    it("returns null for a valid object missing __render (bare path)", () => {
      expect(parseEnvelope('{"data":{"text":"x"}}')).toBeNull();
    });

    it("returns null for a valid object missing __render (fenced path)", () => {
      const content = '```json\n{"data":{"text":"x"}}\n```';
      expect(parseEnvelope(content)).toBeNull();
    });

    it("returns null when __render is a number", () => {
      expect(parseEnvelope('{"__render":1}')).toBeNull();
    });

    it("returns null when __render is an object", () => {
      expect(parseEnvelope('{"__render":{}}')).toBeNull();
    });

    it("returns null when __render is an array", () => {
      expect(parseEnvelope('{"__render":[]}')).toBeNull();
    });

    it("returns null when __render is a boolean", () => {
      expect(parseEnvelope('{"__render":false}')).toBeNull();
    });

    it("returns null when __render is null", () => {
      expect(parseEnvelope('{"__render":null}')).toBeNull();
    });

    it("returns null when __render is an empty string", () => {
      expect(parseEnvelope('{"__render":""}')).toBeNull();
    });

    it("returns null when __render is whitespace only", () => {
      expect(parseEnvelope('{"__render":"   "}')).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // Group 4 — fault tolerance (VAL-PARSE-004)
  // ---------------------------------------------------------------------------
  describe("fault tolerance", () => {
    it("returns null for malformed JSON without throwing", () => {
      expect(() => parseEnvelope('{"__render": translation}')).not.toThrow();
      expect(parseEnvelope('{"__render": translation}')).toBeNull();
    });

    it("returns null for unterminated JSON", () => {
      expect(parseEnvelope('{"__render":"translation"')).toBeNull();
    });

    it("returns null for an unclosed fence (opening ```json only)", () => {
      const content = '```json\n{"__render":"translation","data":{}}';
      expect(parseEnvelope(content)).toBeNull();
    });

    it("returns null when a json fence holds non-JSON text", () => {
      const content = "```json\nthis is not json\n```";
      expect(parseEnvelope(content)).toBeNull();
    });

    it("returns null for an empty string", () => {
      expect(parseEnvelope("")).toBeNull();
    });

    it("returns null for whitespace-only content", () => {
      expect(parseEnvelope("   \n\t  ")).toBeNull();
    });

    it("returns null for null content", () => {
      expect(parseEnvelope(null)).toBeNull();
    });

    it("returns null for undefined content", () => {
      expect(parseEnvelope(undefined)).toBeNull();
    });

    it("never throws across the assorted malformed inputs", () => {
      const inputs = [
        "{",
        "}",
        "```json",
        "```json\n",
        "{not json}",
        '{"__render":}',
        "[",
      ];
      for (const input of inputs) {
        expect(() => parseEnvelope(input)).not.toThrow();
        expect(parseEnvelope(input)).toBeNull();
      }
    });
  });

  // ---------------------------------------------------------------------------
  // Group 5 — data pass-through (VAL-PARSE-005)
  // ---------------------------------------------------------------------------
  describe("data pass-through", () => {
    it("passes through when data is missing (undefined)", () => {
      expect(parseEnvelope('{"__render":"card"}')).toEqual({
        type: "card",
        data: undefined,
      });
    });

    it("passes through a string data payload verbatim", () => {
      expect(parseEnvelope('{"__render":"card","data":"hello"}')).toEqual({
        type: "card",
        data: "hello",
      });
    });

    it("passes through a number data payload verbatim", () => {
      expect(parseEnvelope('{"__render":"card","data":42}')).toEqual({
        type: "card",
        data: 42,
      });
    });

    it("passes through an array data payload verbatim", () => {
      expect(parseEnvelope('{"__render":"card","data":[1,2,3]}')).toEqual({
        type: "card",
        data: [1, 2, 3],
      });
    });

    it("passes through a null data payload verbatim", () => {
      expect(parseEnvelope('{"__render":"card","data":null}')).toEqual({
        type: "card",
        data: null,
      });
    });

    it("passes through inside a fenced block too", () => {
      const content = '```json\n{"__render":"card","data":"x"}\n```';
      expect(parseEnvelope(content)).toEqual({ type: "card", data: "x" });
    });
  });
});
