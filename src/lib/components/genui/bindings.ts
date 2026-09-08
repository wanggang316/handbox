/**
 * The bridge between a custom tool's parameters and the GenUI spec that renders
 * them.
 *
 * A tool's view is a spec whose props read the call's arguments through
 * json-render's `{ $state: "/city" }` expressions, with the arguments seeded as
 * the state model. That makes the parameter list and the spec two halves of one
 * contract, and a binding to a parameter nobody declared is the failure this
 * module exists to catch — at authoring time, in the editor, rather than as a
 * blank card in a conversation.
 *
 * Imports no `.svelte` and no state, so it runs under the plain-Node Vitest
 * environment.
 */

import type { Spec, UIElement } from "@json-render/core";
import type { ToolParam, ToolParamType } from "$lib/types/toolDefinition";

/** A `$state` reference found in a spec, with the element it came from. */
export interface SpecBinding {
  /** Element id in `spec.elements` — how the editor points at the problem. */
  elementId: string;
  /** Prop the expression sits on. */
  prop: string;
  /** Raw path as written, e.g. `/city` or `city`. */
  path: string;
}

/** Whether a prop value is a `{ $state: path }` expression. */
function asStateBinding(value: unknown): string | null {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  const path = (value as Record<string, unknown>)["$state"];
  return typeof path === "string" ? path : null;
}

/**
 * Every `$state` expression in the spec, in element order.
 *
 * Only top-level prop values are inspected: that is the shape the widened
 * catalog schemas accept (see `resolveSpec`'s `allowBindings`), so a binding
 * nested deeper would already have been rejected as an invalid prop.
 */
export function collectStateBindings(spec: Spec): SpecBinding[] {
  const bindings: SpecBinding[] = [];
  for (const [elementId, element] of Object.entries(spec.elements) as [
    string,
    UIElement,
  ][]) {
    const props = (element.props ?? {}) as Record<string, unknown>;
    for (const [prop, value] of Object.entries(props)) {
      const path = asStateBinding(value);
      if (path !== null) {
        bindings.push({ elementId, prop, path });
      }
    }
  }
  return bindings;
}

/**
 * The parameter a binding path names.
 *
 * Paths are single-segment by construction — the state model is exactly the
 * arguments object, one key per declared parameter — and json-render's
 * `getByPath` accepts a path with or without the leading slash, so both spell
 * the same parameter. A deeper path (`/rows/0`) has no parameter of its own and
 * yields `null`; the editor reports it rather than silently accepting it.
 */
export function bindingParamName(path: string): string | null {
  const trimmed = path.startsWith("/") ? path.slice(1) : path;
  if (trimmed.length === 0 || trimmed.includes("/")) {
    return null;
  }
  return trimmed;
}

/** The path a parameter is bound by; what the editor shows next to each row. */
export function paramBindingPath(paramName: string): string {
  return `/${paramName}`;
}

export interface BindingIssue {
  binding: SpecBinding;
  /** Human-readable reason, ready to show on the form. */
  message: string;
}

/**
 * Bindings that name something the tool does not declare.
 *
 * Returned rather than thrown so the editor can list every problem at once; an
 * empty array means the view and the parameter list agree.
 */
export function validateBindings(
  spec: Spec,
  parameters: ToolParam[],
): BindingIssue[] {
  const declared = new Set(parameters.map((param) => param.name));
  const issues: BindingIssue[] = [];

  for (const binding of collectStateBindings(spec)) {
    const paramName = bindingParamName(binding.path);
    if (paramName === null) {
      issues.push({
        binding,
        message: `"${binding.elementId}" 的 ${binding.prop} 绑定了 ${binding.path}，只能绑定到一个参数（如 /city）`,
      });
      continue;
    }
    if (!declared.has(paramName)) {
      issues.push({
        binding,
        message: `"${binding.elementId}" 的 ${binding.prop} 绑定了未声明的参数 ${paramName}`,
      });
    }
  }

  return issues;
}

/** Placeholder value per parameter type, shaped so the catalog can render it. */
function sampleValue(param: ToolParam): unknown {
  switch (param.type) {
    case "string":
      return param.description.trim() || param.name;
    case "number":
      return 42;
    case "boolean":
      return true;
    case "stringList":
      return [`${param.name} 1`, `${param.name} 2`];
    case "stringMatrix":
      return [
        ["A1", "B1"],
        ["A2", "B2"],
      ];
    case "keyValueList":
      return [
        { key: "键", value: "值" },
        { key: "键 2", value: "值 2" },
      ];
    default:
      return unreachable(param.type);
  }
}

/**
 * Stand-in arguments for the editor's preview, so an author sees their layout
 * with content in it instead of an empty card. Every declared parameter gets a
 * value — including the optional ones, since the point is to see the full
 * layout, not a representative call.
 */
export function sampleStateFromParams(
  parameters: ToolParam[],
): Record<string, unknown> {
  return Object.fromEntries(
    parameters.map((param) => [param.name, sampleValue(param)]),
  );
}

/** Compile-time exhaustiveness guard over {@link ToolParamType}. */
function unreachable(type: never): never {
  throw new Error(`unhandled tool parameter type: ${type as ToolParamType}`);
}
