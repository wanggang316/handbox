/**
 * "Open in ..." API —— 在外部 editor / terminal / 系统文件管理器中打开工作目录。
 *
 * 探测与启动都在后端完成（commands/open_in.rs）：前端只拿到目标清单并回灌 id。
 */

import { apiCall } from "./index";

/** 一个可在下拉中选择的「打开目标」（镜像后端 `OpenInTarget`）。 */
export interface OpenInTarget {
  /** 稳定 id，回灌给 `openInTarget` 解析启动方式（`"system"` = Finder/文件管理器）。 */
  id: string;
  /** 展示名（如 "Visual Studio Code"）。 */
  name: string;
  /** 分类，用于分组 / 选图标。 */
  kind: "editor" | "terminal" | "system";
  /** 应用图标（`data:image/png;base64,...`）；取不到时为 null，前端回退到内置图标。 */
  icon?: string | null;
}

/** 列出当前系统可用的打开目标（已安装的 editor/terminal + 系统文件管理器）。 */
export async function listOpenInTargets(): Promise<OpenInTarget[]> {
  return apiCall<OpenInTarget[]>("open_in_list_targets");
}

/** 在指定 target 中打开目录（`path` 须为已存在目录）。 */
export async function openInTarget(
  path: string,
  targetId: string,
): Promise<void> {
  return apiCall<void>("open_in_open", { path, targetId });
}
