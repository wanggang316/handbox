/**
 * Mirrors the backend `storage/types/agent_project.rs` (serde camelCase).
 * `path` is the canonicalized working directory (canonicalization happens in
 * the backend service layer); timestamps are millisecond i64.
 */

import type { UUID, Timestamp } from "./index";

/** Groups agent-mode sessions by working directory. */
export interface AgentProject {
  id: UUID;
  path: string;
  name: string;
  /** Floats the project to the top of the sidebar's project section. */
  pinned: boolean;
  /** `#rrggbb` tint for the sidebar row's name; null = the default color. */
  color?: string | null;
  /** Per-project "Open in ..." target id; null = follow the global default. */
  defaultEditorId?: string | null;
  createdAt: Timestamp;
  updatedAt: Timestamp;
}

/** The settings panel's fields, written as one group (null = unset). */
export interface AgentProjectSettings {
  name: string;
  color: string | null;
  defaultEditorId: string | null;
}
