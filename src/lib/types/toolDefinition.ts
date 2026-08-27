/**
 * User-defined agent tools ("dynamic tools").
 * Mirrors `src-tauri/src/storage/types/tool_definition.rs`.
 *
 * A definition is PRESENTATIONAL: the tool performs no work. The model calls it
 * with the declared parameters, and the timeline renders the linked GenUI spec
 * with those arguments as its state model — so a parameter's name doubles as the
 * binding path a spec reads (`{ "$state": "/city" }`).
 */

/**
 * The type of one declared parameter.
 *
 * Closed set, aligned with the GenUI catalog's prop shapes
 * (`components/genui/jsonui/catalog.ts`): a parameter exists to be bound into a
 * spec, so a type no component can consume would be a parameter nobody can use.
 */
export type ToolParamType =
  | "string"
  | "number"
  | "boolean"
  /** `string[]` — e.g. `Table.columns`. */
  | "stringList"
  /** `string[][]` — e.g. `Table.rows`. */
  | "stringMatrix"
  /** `{key, value}[]` — e.g. `KeyValue.items`. */
  | "keyValueList";

export interface ToolParam {
  /** Identifier; also the state path a GenUI spec binds to (`/city`). */
  name: string;
  type: ToolParamType;
  /** Shown to the model inside the schema — what teaches it what to put here. */
  description: string;
  required: boolean;
}

export interface ToolDefinition {
  id: string;
  /** Registration name the model calls; unique, and never a built-in id. */
  name: string;
  /** Human label for the UI. */
  displayName: string;
  /** Lucide icon name (kebab-case); resolve via `resolveAgentIcon`. */
  icon: string | null;
  /** The prompt the model reads: when and why to call this tool. */
  description: string;
  parameters: ToolParam[];
  /** Linked GenUI spec rendered from the call's arguments. */
  genuiId: string | null;
  enabled: boolean;
  /** Display order in settings; also the registration order in a run. */
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
}

export interface CreateToolDefinitionRequest {
  name: string;
  displayName: string;
  icon?: string | null;
  description?: string;
  parameters?: ToolParam[];
  genuiId?: string | null;
  sortOrder?: number | null;
}

/**
 * Every field optional; an omitted field keeps its stored value. For the two
 * nullable fields (`icon`, `genuiId`) an **empty string clears** them.
 */
export interface UpdateToolDefinitionRequest {
  name?: string;
  displayName?: string;
  icon?: string;
  description?: string;
  parameters?: ToolParam[];
  genuiId?: string;
  enabled?: boolean;
  sortOrder?: number;
}
