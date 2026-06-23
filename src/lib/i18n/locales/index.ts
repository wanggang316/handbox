/**
 * 语言词典装配（仅在此文件集中维护命名空间接线）。
 *
 * 约定：
 * - `zh-CN` 为权威词典，决定全部 key（`MessageKey`）。
 * - 各命名空间的 en 文件在自身内通过 `Record<keyof typeof <ns>Zh, string>`
 *   做**本地**完整性校验——漏译会在对应 en 文件就地报错，互不耦合，
 *   因此多个迁移代理可并行工作、各自独立 `npm run check`。
 * - 新增命名空间时，仅需在此文件 import + spread 两处。
 */
import { commonZh } from "./zh/common";
import { sidebarZh } from "./zh/sidebar";
import { settingsZh } from "./zh/settings";
import { agentZh } from "./zh/agent";
import { chatZh } from "./zh/chat";
import { jobsZh } from "./zh/jobs";
import { wordsZh } from "./zh/words";
import { favoritesZh } from "./zh/favorites";
import { artifactsZh } from "./zh/artifacts";
import { updateZh } from "./zh/update";
import { selectionZh } from "./zh/selection";
import { uiZh } from "./zh/ui";
import { providerZh } from "./zh/provider";

import { commonEn } from "./en/common";
import { sidebarEn } from "./en/sidebar";
import { settingsEn } from "./en/settings";
import { agentEn } from "./en/agent";
import { chatEn } from "./en/chat";
import { jobsEn } from "./en/jobs";
import { wordsEn } from "./en/words";
import { favoritesEn } from "./en/favorites";
import { artifactsEn } from "./en/artifacts";
import { updateEn } from "./en/update";
import { selectionEn } from "./en/selection";
import { uiEn } from "./en/ui";
import { providerEn } from "./en/provider";

// 权威词典：key 的唯一来源。
const zh = {
  ...commonZh,
  ...sidebarZh,
  ...settingsZh,
  ...agentZh,
  ...chatZh,
  ...jobsZh,
  ...wordsZh,
  ...favoritesZh,
  ...artifactsZh,
  ...updateZh,
  ...selectionZh,
  ...uiZh,
  ...providerZh,
};

const en = {
  ...commonEn,
  ...sidebarEn,
  ...settingsEn,
  ...agentEn,
  ...chatEn,
  ...jobsEn,
  ...wordsEn,
  ...favoritesEn,
  ...artifactsEn,
  ...updateEn,
  ...selectionEn,
  ...uiEn,
  ...providerEn,
};

export type Locale = "zh-CN" | "en-US";
export type MessageKey = keyof typeof zh;

// 值类型放宽，避免在此处引入跨命名空间的全局耦合；
// 完整性由各 en 命名空间文件本地保证。
export const dictionaries: Record<Locale, Record<string, string>> = {
  "zh-CN": zh,
  "en-US": en,
};
