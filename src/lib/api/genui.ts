/**
 * GenUI 相关 API 封装
 *
 * GenUI 是一份具名、可复用的 JSON-Render UI spec。后端把 `spec` 当作不透明文本存储，
 * 前端在保存前自行用 explainSpec 校验。
 */

import { apiCall } from "./index";
import type { GenUi, UUID } from "../types";

/**
 * 创建新的 GenUI
 * 后端签名: genui_create(request: CreateGenUiRequest)
 */
export async function createGenui(name: string, spec: string): Promise<GenUi> {
  return apiCall<GenUi>("genui_create", { request: { name, spec } });
}

/**
 * 获取 GenUI 列表（按 updatedAt 倒序）
 */
export async function getGenuis(
  limit?: number,
  offset?: number,
): Promise<GenUi[]> {
  return apiCall<GenUi[]>("genui_list", { limit, offset });
}

/**
 * 获取 GenUI 详情
 */
export async function getGenui(genuiId: UUID): Promise<GenUi> {
  return apiCall<GenUi>("genui_get", { genuiId });
}

/**
 * 更新 GenUI（名称 / spec 按需更新）
 * 后端签名: genui_update(genui_id, request: UpdateGenUiRequest)
 */
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

/**
 * 删除 GenUI（同时把引用它的 agent.genuiId 置空，由后端处理）
 */
export async function deleteGenui(genuiId: UUID): Promise<void> {
  return apiCall<void>("genui_delete", { genuiId });
}
