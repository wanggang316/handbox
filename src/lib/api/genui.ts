/**
 * A GenUI is a named, reusable JSON-Render UI spec. The backend stores `spec`
 * as opaque text; callers validate it with explainSpec before saving.
 */

import { apiCall } from "./index";
import type { GenUi, UUID } from "../types";

export async function createGenui(name: string, spec: string): Promise<GenUi> {
  return apiCall<GenUi>("genui_create", { request: { name, spec } });
}

/** Ordered by updatedAt descending. */
export async function getGenuis(
  limit?: number,
  offset?: number,
): Promise<GenUi[]> {
  return apiCall<GenUi[]>("genui_list", { limit, offset });
}

export async function getGenui(genuiId: UUID): Promise<GenUi> {
  return apiCall<GenUi>("genui_get", { genuiId });
}

/** Omitted fields are left unchanged. */
export async function updateGenui(
  genuiId: UUID,
  name?: string,
  spec?: string,
): Promise<GenUi> {
  return apiCall<GenUi>("genui_update", {
    genuiId,
    request: { name, spec },
  });
}

/** The backend also clears any agent.genuiId referencing it. */
export async function deleteGenui(genuiId: UUID): Promise<void> {
  return apiCall<void>("genui_delete", { genuiId });
}
