/**
 * Unit tests for the tool-parameter ↔ GenUI binding helpers, and for the
 * binding-tolerant prop validation they depend on.
 *
 * Pure TypeScript — no `.svelte` imports — so this suite runs under the
 * plain-Node Vitest environment.
 */

import { describe, it, expect } from "vitest";
import type { Spec } from "@json-render/core";
import {
  bindingParamName,
  collectStateBindings,
  paramBindingPath,
  sampleStateFromParams,
  validateBindings,
} from "./bindings";
import { explainSpec, resolveSpec } from "./jsonui/resolveSpec";
import type { ToolParam, ToolParamType } from "$lib/types/toolDefinition";

function param(name: string, type: ToolParamType = "string"): ToolParam {
  return { name, type, description: "", required: false };
}

/** A spec binding `Text.text` to `/city`, as a tool's view would. */
function boundSpecText(): string {
  return JSON.stringify({
    root: "card",
    elements: {
      card: {
        type: "Card",
        props: { title: "Weather" },
        children: ["label"],
        visible: true,
      },
      label: {
        type: "Text",
        props: { text: { $state: "/city" } },
        children: [],
        visible: true,
      },
    },
  });
}

describe("binding-tolerant prop validation", () => {
  it("rejects a $state binding by default — model output must be literal", () => {
    expect(resolveSpec(boundSpecText())).toBeNull();
    const diagnostic = explainSpec(boundSpecText());
    expect(diagnostic.ok).toBe(false);
    if (!diagnostic.ok) expect(diagnostic.stage).toBe("props");
  });

  it("accepts a $state binding with allowBindings and keeps it intact", () => {
    const spec = resolveSpec(boundSpecText(), { allowBindings: true });
    expect(spec).not.toBeNull();
    expect(spec!.elements.label.props).toEqual({ text: { $state: "/city" } });
  });

  /// Widening must not turn into "anything goes": the guarantees the strict
  /// path provides have to survive.
  it("still enforces required props, literal types and the binding's own shape", () => {
    const withOptions = (spec: unknown) =>
      explainSpec(JSON.stringify(spec), { allowBindings: true });

    const missingRequired = withOptions({
      root: "t",
      elements: { t: { type: "Text", props: {}, children: [], visible: true } },
    });
    expect(missingRequired.ok).toBe(false);

    const wrongLiteral = withOptions({
      root: "t",
      elements: {
        t: { type: "Text", props: { text: 42 }, children: [], visible: true },
      },
    });
    expect(wrongLiteral.ok).toBe(false);

    const malformedBinding = withOptions({
      root: "t",
      elements: {
        t: {
          type: "Text",
          props: { text: { $state: 42 } },
          children: [],
          visible: true,
        },
      },
    });
    expect(malformedBinding.ok).toBe(false);
  });

  it("binds an enum prop as readily as a string one", () => {
    const spec = resolveSpec(
      JSON.stringify({
        root: "s",
        elements: {
          s: {
            type: "StatusLabel",
            props: { status: { $state: "/state" }, text: "Build" },
            children: [],
            visible: true,
          },
        },
      }),
      { allowBindings: true },
    );
    expect(spec).not.toBeNull();
  });
});

describe("collectStateBindings", () => {
  it("finds every binding with the element and prop it sits on", () => {
    const spec = resolveSpec(boundSpecText(), { allowBindings: true }) as Spec;
    expect(collectStateBindings(spec)).toEqual([
      { elementId: "label", prop: "text", path: "/city" },
    ]);
  });

  it("is empty for a spec of literals", () => {
    const spec = resolveSpec(
      JSON.stringify({
        root: "t",
        elements: {
          t: { type: "Text", props: { text: "hi" }, children: [], visible: true },
        },
      }),
    ) as Spec;
    expect(collectStateBindings(spec)).toEqual([]);
  });
});

describe("bindingParamName", () => {
  it("accepts a single segment with or without the leading slash", () => {
    expect(bindingParamName("/city")).toBe("city");
    expect(bindingParamName("city")).toBe("city");
  });

  it("rejects a deeper path or an empty one — no parameter names those", () => {
    expect(bindingParamName("/rows/0")).toBeNull();
    expect(bindingParamName("/")).toBeNull();
    expect(bindingParamName("")).toBeNull();
  });

  it("round-trips with paramBindingPath", () => {
    expect(bindingParamName(paramBindingPath("city"))).toBe("city");
  });
});

describe("validateBindings", () => {
  it("passes when every binding names a declared parameter", () => {
    const spec = resolveSpec(boundSpecText(), { allowBindings: true }) as Spec;
    expect(validateBindings(spec, [param("city")])).toEqual([]);
  });

  /// The failure this module exists for: a card that renders blank in a real
  /// conversation because the view and the parameter list drifted apart.
  it("reports a binding to an undeclared parameter", () => {
    const spec = resolveSpec(boundSpecText(), { allowBindings: true }) as Spec;
    const issues = validateBindings(spec, [param("town")]);
    expect(issues).toHaveLength(1);
    expect(issues[0].binding.elementId).toBe("label");
    expect(issues[0].message).toContain("city");
  });

  it("reports a path that no single parameter can satisfy", () => {
    const spec = resolveSpec(
      JSON.stringify({
        root: "t",
        elements: {
          t: {
            type: "Text",
            props: { text: { $state: "/rows/0" } },
            children: [],
            visible: true,
          },
        },
      }),
      { allowBindings: true },
    ) as Spec;
    expect(validateBindings(spec, [param("rows")])).toHaveLength(1);
  });
});

describe("sampleStateFromParams", () => {
  it("produces a value per parameter, shaped for the type", () => {
    const sample = sampleStateFromParams([
      param("city", "string"),
      param("temp", "number"),
      param("live", "boolean"),
      param("columns", "stringList"),
      param("rows", "stringMatrix"),
      param("items", "keyValueList"),
    ]);

    expect(typeof sample.city).toBe("string");
    expect(typeof sample.temp).toBe("number");
    expect(typeof sample.live).toBe("boolean");
    expect(sample.columns).toEqual(expect.arrayContaining([expect.any(String)]));
    expect(Array.isArray((sample.rows as string[][])[0])).toBe(true);
    expect(sample.items).toEqual([
      { key: expect.any(String), value: expect.any(String) },
      { key: expect.any(String), value: expect.any(String) },
    ]);
  });

  it("is empty for a tool with no parameters", () => {
    expect(sampleStateFromParams([])).toEqual({});
  });
});
