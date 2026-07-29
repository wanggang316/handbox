/**
 * Dictionary assembly — namespace wiring lives only in this file.
 *
 * Conventions:
 * - `zh-CN` is the canonical dictionary and defines every key (`MessageKey`).
 * - Each en namespace file checks its own completeness locally via
 *   `Record<keyof typeof <ns>Zh, string>`, so a missing translation errors
 *   in that file without cross-namespace coupling.
 * - Adding a namespace only requires an import + spread here.
 */
import { commonZh } from "./zh/common";
import { sidebarZh } from "./zh/sidebar";
import { settingsZh } from "./zh/settings";
import { agentZh } from "./zh/agent";
import { jobsZh } from "./zh/jobs";
import { updateZh } from "./zh/update";
import { selectionZh } from "./zh/selection";
import { uiZh } from "./zh/ui";
import { providerZh } from "./zh/provider";
import { quickactionZh } from "./zh/quickaction";

import { commonEn } from "./en/common";
import { sidebarEn } from "./en/sidebar";
import { settingsEn } from "./en/settings";
import { agentEn } from "./en/agent";
import { jobsEn } from "./en/jobs";
import { updateEn } from "./en/update";
import { selectionEn } from "./en/selection";
import { uiEn } from "./en/ui";
import { providerEn } from "./en/provider";
import { quickactionEn } from "./en/quickaction";

// Canonical dictionary: the single source of keys.
const zh = {
  ...commonZh,
  ...sidebarZh,
  ...settingsZh,
  ...agentZh,
  ...jobsZh,
  ...updateZh,
  ...selectionZh,
  ...uiZh,
  ...providerZh,
  ...quickactionZh,
};

const en = {
  ...commonEn,
  ...sidebarEn,
  ...settingsEn,
  ...agentEn,
  ...jobsEn,
  ...updateEn,
  ...selectionEn,
  ...uiEn,
  ...providerEn,
  ...quickactionEn,
};

export type Locale = "zh-CN" | "en-US";
export type MessageKey = keyof typeof zh;

// Value types are widened to avoid cross-namespace coupling here;
// completeness is enforced locally by each en namespace file.
export const dictionaries: Record<Locale, Record<string, string>> = {
  "zh-CN": zh,
  "en-US": en,
};
