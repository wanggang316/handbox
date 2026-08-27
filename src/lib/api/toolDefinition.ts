/**
 * User-defined tool ("dynamic tool") CRUD.
 *
 * The backend owns the naming rules: a registration name must be a lower-snake
 * identifier, unique, and free of the built-in / `mcp__*` namespaces — so a
 * rejected save arrives as a `VALIDATION_ERROR` AppError to show on the field.
 */

import { apiCall } from "./index";
import type {
  CreateToolDefinitionRequest,
  ToolDefinition,
  UpdateToolDefinitionRequest,
} from "../types";

/** Every definition, enabled or not, in display order. */
export async function listToolDefinitions(): Promise<ToolDefinition[]> {
  return apiCall<ToolDefinition[]>("tool_definition_list");
}

export async function getToolDefinition(
  toolId: string,
): Promise<ToolDefinition> {
  return apiCall<ToolDefinition>("tool_definition_get", { toolId });
}

export async function createToolDefinition(
  request: CreateToolDefinitionRequest,
): Promise<ToolDefinition> {
  return apiCall<ToolDefinition>("tool_definition_create", { request });
}

/** Omitted fields are left unchanged; `icon`/`genuiId` clear on empty string. */
export async function updateToolDefinition(
  toolId: string,
  request: UpdateToolDefinitionRequest,
): Promise<ToolDefinition> {
  return apiCall<ToolDefinition>("tool_definition_update", { toolId, request });
}

export async function deleteToolDefinition(toolId: string): Promise<void> {
  await apiCall<void>("tool_definition_delete", { toolId });
}
