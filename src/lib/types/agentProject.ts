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
  createdAt: Timestamp;
  updatedAt: Timestamp;
}
