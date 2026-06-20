/**
 * GenUI 相关类型定义 - 匹配后端 Rust 架构
 *
 * GenUI 是一份具名、可复用的 JSON-Render UI spec。`spec` 是原样的 spec JSON 文本
 * （保存前由前端经 explainSpec 校验），后端视其为不透明字符串、从不解析其结构。
 */

import type { BaseEntity } from "./index";

// GenUI 实体
export interface GenUi extends BaseEntity {
  name: string;
  spec: string;
}

// 创建 GenUI 请求
export interface CreateGenUiRequest {
  name: string;
  spec: string;
}

// 更新 GenUI 请求（名称 / spec 按需更新）
export interface UpdateGenUiRequest {
  name?: string;
  spec?: string;
}
