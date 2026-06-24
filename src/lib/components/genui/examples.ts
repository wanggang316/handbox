/**
 * Curated GenUI starter templates.
 *
 * Each example is a valid JSON-Render {@link Spec} composed from the shared
 * presentational catalog (Card / Text / Badge / Stack / StatusLabel / Avatar /
 * Divider / KeyValue / Table / InfoTooltip). They render through the same
 * `resolveSpec → Renderer` pipeline as chat, so the editor's live preview and
 * diagnostics apply unchanged.
 *
 * Used by `GenUiEditor` as "load an example" starting points; kept as a plain
 * data module (no `.svelte`) so it can be reused elsewhere (gallery, agent
 * few-shot defaults) without pulling in component code.
 */

import type { Spec } from "@json-render/core";

export interface GenUiExample {
  /** Stable id used as the picker option value. */
  id: string;
  /** Human-facing template name (also pre-fills the editor's name field). */
  name: string;
  /** One-line description of what the template is for. */
  description: string;
  /** The renderable spec. */
  spec: Spec;
}

/**
 * Common layout patterns, ordered roughly simplest → richest. Every required
 * prop is present and every `children` id resolves, so each passes resolveSpec.
 */
export const genuiExamples: GenUiExample[] = [
  {
    id: "translation",
    name: "翻译卡",
    description: "原文 / 译文 / 语言三行对照，最常见的翻译输出。",
    spec: {
      root: "card",
      elements: {
        card: {
          type: "Card",
          props: { title: "翻译" },
          children: ["body"],
          visible: true,
        },
        body: {
          type: "Stack",
          props: { gap: "md" },
          children: ["kv", "note"],
          visible: true,
        },
        kv: {
          type: "KeyValue",
          props: {
            items: [
              { key: "原文", value: "Hello, world" },
              { key: "译文", value: "你好，世界" },
              { key: "语言", value: "英语 → 中文" },
            ],
          },
          children: [],
          visible: true,
        },
        note: {
          type: "Text",
          props: { text: "把上面的字段替换为真实翻译内容。", variant: "muted" },
          children: [],
          visible: true,
        },
      },
    },
  },
  {
    id: "profile",
    name: "资料卡",
    description: "头像 + 姓名 + 角色 + 在线状态，适合人物 / 实体简介。",
    spec: {
      root: "card",
      elements: {
        card: { type: "Card", props: {}, children: ["row"], visible: true },
        row: {
          type: "Stack",
          props: { direction: "row", gap: "md" },
          children: ["avatar", "info"],
          visible: true,
        },
        avatar: {
          type: "Avatar",
          props: { letter: "A", size: "lg" },
          children: [],
          visible: true,
        },
        info: {
          type: "Stack",
          props: { gap: "sm" },
          children: ["name", "role", "status"],
          visible: true,
        },
        name: {
          type: "Text",
          props: { text: "Ada Lovelace", variant: "heading" },
          children: [],
          visible: true,
        },
        role: {
          type: "Text",
          props: { text: "首席工程师", variant: "muted" },
          children: [],
          visible: true,
        },
        status: {
          type: "StatusLabel",
          props: { status: "enabled", text: "在线" },
          children: [],
          visible: true,
        },
      },
    },
  },
  {
    id: "status-panel",
    name: "状态面板",
    description: "多条状态标签 + 关键指标，适合健康检查 / 运行概况。",
    spec: {
      root: "card",
      elements: {
        card: {
          type: "Card",
          props: { title: "系统状态" },
          children: ["stack"],
          visible: true,
        },
        stack: {
          type: "Stack",
          props: { gap: "md" },
          children: ["s1", "s2", "s3", "div", "kv"],
          visible: true,
        },
        s1: {
          type: "StatusLabel",
          props: { status: "enabled", text: "API 服务正常" },
          children: [],
          visible: true,
        },
        s2: {
          type: "StatusLabel",
          props: { status: "idle", text: "后台任务空闲" },
          children: [],
          visible: true,
        },
        s3: {
          type: "StatusLabel",
          props: { status: "error", text: "数据库连接失败" },
          children: [],
          visible: true,
        },
        div: { type: "Divider", props: {}, children: [], visible: true },
        kv: {
          type: "KeyValue",
          props: {
            items: [
              { key: "运行时长", value: "3 天 12 小时" },
              { key: "版本", value: "v1.4.2" },
            ],
          },
          children: [],
          visible: true,
        },
      },
    },
  },
  {
    id: "table",
    name: "数据表格",
    description: "标题 + 只读表格，适合罗列结构化数据。",
    spec: {
      root: "card",
      elements: {
        card: {
          type: "Card",
          props: { title: "本月销售" },
          children: ["stack"],
          visible: true,
        },
        stack: {
          type: "Stack",
          props: { gap: "sm" },
          children: ["intro", "table"],
          visible: true,
        },
        intro: {
          type: "Text",
          props: { text: "各地区销售额（单位：万元）", variant: "muted" },
          children: [],
          visible: true,
        },
        table: {
          type: "Table",
          props: {
            columns: ["地区", "销售额", "环比"],
            rows: [
              ["华东", "128", "+12%"],
              ["华南", "96", "-3%"],
              ["华北", "75", "+8%"],
            ],
          },
          children: [],
          visible: true,
        },
      },
    },
  },
  {
    id: "badges",
    name: "标签集合",
    description: "一行四色徽章，演示 info / success / warning / error 语义。",
    spec: {
      root: "card",
      elements: {
        card: {
          type: "Card",
          props: { title: "标签" },
          children: ["row"],
          visible: true,
        },
        row: {
          type: "Stack",
          props: { direction: "row", gap: "sm" },
          children: ["b1", "b2", "b3", "b4"],
          visible: true,
        },
        b1: {
          type: "Badge",
          props: { label: "信息", tone: "info" },
          children: [],
          visible: true,
        },
        b2: {
          type: "Badge",
          props: { label: "成功", tone: "success" },
          children: [],
          visible: true,
        },
        b3: {
          type: "Badge",
          props: { label: "警告", tone: "warning" },
          children: [],
          visible: true,
        },
        b4: {
          type: "Badge",
          props: { label: "错误", tone: "error" },
          children: [],
          visible: true,
        },
      },
    },
  },
  {
    id: "detail",
    name: "键值详情",
    description: "键值列表 + 悬浮说明，适合订单 / 条目详情。",
    spec: {
      root: "card",
      elements: {
        card: {
          type: "Card",
          props: { title: "订单详情" },
          children: ["stack"],
          visible: true,
        },
        stack: {
          type: "Stack",
          props: { gap: "md" },
          children: ["kv", "hintRow"],
          visible: true,
        },
        kv: {
          type: "KeyValue",
          props: {
            items: [
              { key: "订单号", value: "#100245" },
              { key: "金额", value: "¥299.00" },
              { key: "状态", value: "已发货" },
              { key: "收货人", value: "张三" },
            ],
          },
          children: [],
          visible: true,
        },
        hintRow: {
          type: "Stack",
          props: { direction: "row", gap: "sm" },
          children: ["hintText", "tip"],
          visible: true,
        },
        hintText: {
          type: "Text",
          props: { text: "配送时效说明", variant: "muted" },
          children: [],
          visible: true,
        },
        tip: {
          type: "InfoTooltip",
          props: { content: "标准配送 3–5 个工作日，偏远地区可能延迟。" },
          children: [],
          visible: true,
        },
      },
    },
  },
  {
    id: "dictionary",
    name: "词典释义卡",
    description: "词条 + 词性徽章 + 释义 + 例句，适合词典 / 解释类输出。",
    spec: {
      root: "card",
      elements: {
        card: { type: "Card", props: {}, children: ["stack"], visible: true },
        stack: {
          type: "Stack",
          props: { gap: "sm" },
          children: ["head", "def", "div", "kv"],
          visible: true,
        },
        head: {
          type: "Stack",
          props: { direction: "row", gap: "sm" },
          children: ["word", "pos"],
          visible: true,
        },
        word: {
          type: "Text",
          props: { text: "ephemeral", variant: "heading" },
          children: [],
          visible: true,
        },
        pos: {
          type: "Badge",
          props: { label: "adj.", tone: "info" },
          children: [],
          visible: true,
        },
        def: {
          type: "Text",
          props: { text: "持续时间极短的；短暂的。", variant: "body" },
          children: [],
          visible: true,
        },
        div: { type: "Divider", props: {}, children: [], visible: true },
        kv: {
          type: "KeyValue",
          props: {
            items: [
              { key: "例句", value: "Ephemeral pleasures fade fast." },
              { key: "近义", value: "transient, fleeting" },
            ],
          },
          children: [],
          visible: true,
        },
      },
    },
  },
  {
    id: "overview",
    name: "概览卡",
    description: "一行多指标 + 分隔线 + 备注，适合 KPI / 数据概览。",
    spec: {
      root: "card",
      elements: {
        card: {
          type: "Card",
          props: { title: "今日概览" },
          children: ["stack"],
          visible: true,
        },
        stack: {
          type: "Stack",
          props: { gap: "md" },
          children: ["row", "div", "note"],
          visible: true,
        },
        row: {
          type: "Stack",
          props: { direction: "row", gap: "lg" },
          children: ["k1", "k2", "k3"],
          visible: true,
        },
        k1: {
          type: "KeyValue",
          props: { items: [{ key: "访客", value: "1,204" }] },
          children: [],
          visible: true,
        },
        k2: {
          type: "KeyValue",
          props: { items: [{ key: "订单", value: "86" }] },
          children: [],
          visible: true,
        },
        k3: {
          type: "KeyValue",
          props: { items: [{ key: "转化率", value: "7.1%" }] },
          children: [],
          visible: true,
        },
        div: { type: "Divider", props: {}, children: [], visible: true },
        note: {
          type: "Text",
          props: { text: "数据较昨日 +5.2%", variant: "muted" },
          children: [],
          visible: true,
        },
      },
    },
  },
];
