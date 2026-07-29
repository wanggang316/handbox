/**
 * A GenUI is a named, reusable JSON-Render UI spec. `spec` is the raw spec
 * JSON text (validated by the frontend via explainSpec before saving); the
 * backend treats it as an opaque string and never parses it.
 */

import type { BaseEntity } from "./index";

export interface GenUi extends BaseEntity {
  name: string;
  spec: string;
}

export interface CreateGenUiRequest {
  name: string;
  spec: string;
}

export interface UpdateGenUiRequest {
  name?: string;
  spec?: string;
}
