# hand-ai 模型 metadata 扩展提案

**Author:** Claude (HandBox 侧)
**Audience:** hand-ai 维护者
**Status:** Draft, awaiting review
**Date:** 2026-05-27
**Source:** HandBox `dissolve-handbox-llm` 之后的 catalog gap 审计（详见 `docs/exec-plans/dissolve-handbox-llm.md`）

## 1. 背景

HandBox 通过 `hand_ai_model 0.2.0` 替换了内部 `handbox-llm` crate（35 个 commit、净删除 ~4300 行 Rust）。chat dispatch、provider catalog、capabilities、wrapper-level cancellation 全部走 hand-ai。

替换之后 HandBox 端仍保留一份 `src-tauri/llm_config.json`，用于 UI 元数据（slider 组件、tooltip、默认值等）。其中**绝大部分是产品域**（中文文案、UI 组件类型、UX 默认值），不需要 hand-ai 关心。

但有 3 类**协议/模型事实**目前夹在 HandBox JSON 里，本质上应该归 hand-ai 管。本提案列出这些 gap 与建议的 API 扩展。

## 2. 提案概览

| # | Gap | hand-ai 现状 | 建议变更 | 估算工作量 |
|---|-----|------|------|------|
| **P1** | gpt-5-pro 仅支持 `high` effort，其他 gpt-5 / o-系列也有类似 effort 子集 | 386 个 reasoning 模型里只有 2 个 deepseek 带 `thinking_level_map` | 数据填充：给受限 OpenAI 系列补 `thinkingLevelMap` | 数据-only（无 API 变更） |
| **P2** | Gemini 2.5 系列每个模型有不同的 thinking-token-budget 区间（pro: 128-32768; flash: 1-24576; flash-lite: 512-24576），HandBox JSON 是唯一来源 | `ThinkingBudgets` 只表达 per-level 固定 budget，无法表达 user-pickable range | 新增 `Model.thinking_token_budget: Option<TokenBudgetRange>` | API 扩展 + 数据填充 |
| **P3** | OpenAI Responses 的 `reasoning.summary` 字段（`auto`/`detailed`/`concise`）目前 hand-ai 完全没建模 | 无 | 在 `ApiCapabilities` 加 `reasoning_summary_modes`；optional per-model override | API 扩展 + 数据填充 |
| **P4** *(defer)* | OpenRouter 上的 anthropic-prefixed 模型走 `max_tokens` reasoning prop 而非 effort | OpenRouter 所有模型 `api=openai-completions` 一刀切；hand-ai 看不到底层 vendor 差异 | 需要新的 model-level "reasoning protocol" 字段 | 大改动，建议先观察 |

P1 / P2 / P3 建议分三个独立 PR 推进；它们之间无依赖。P4 是 cross-cutting redesign，先 defer。

---

## 3. P1：填充 `thinking_level_map` 数据

### 现状

hand-ai `get_supported_thinking_levels(model)` 已经实现了正确的语义（`models.rs:148-170`）：

- `thinking_level_map` 中 key 对应 `Some(None)`（JSON `null`）→ **该 level 不支持**
- key 对应 `Some(Some(format_str))` → **支持**，wire payload 用 format_str
- key 缺失 → **支持**（除 `xhigh` 走 substring 启发式）
- 整个 map 为 None → **全部支持**（`xhigh` 走 substring 启发式）

但**只有 2 个 deepseek 模型实际填了 `thinkingLevelMap`**（`deepseek-v4-flash`、`deepseek-v4-pro`）。其他 384 个 reasoning 模型全为 `null`。

### 目标变更

**Data-only.** 给 effort 受限的 OpenAI 系列模型补 `thinkingLevelMap`，让 `get_supported_thinking_levels` 直接给出正确答案，下游 embedder（HandBox 等）就不用维护"per-model effort exception"列表了。

### 已知需要 restriction 的模型（来自 HandBox 数据 + hand-ai catalog 审计）

#### 3.1 `openai/gpt-5-pro`（含 azure/openrouter/vercel 镜像）

HandBox `llm_config.json` 显式声明 `effort_options."openai/gpt-5-pro" = ["high"]`。即 minimal/low/medium 都不受支持。

hand-ai catalog 中相关条目（5 个）：

| Provider | Model id | API |
|---|---|---|
| `openai` | `gpt-5-pro` | `openai-responses` |
| `azure-openai-responses` | `gpt-5-pro` | `azure-openai-responses` |
| `openrouter` | `openai/gpt-5-pro` | `openai-completions` |
| `vercel-ai-gateway` | `openai/gpt-5-pro` | `anthropic-messages` |

建议 `thinkingLevelMap`：

```json
{
  "minimal": null,
  "low": null,
  "medium": null,
  "high": "high",
  "xhigh": null
}
```

`high` 值为 `"high"`（按 OpenAI 协议字面值）；其他全 `null`。

#### 3.2 其他候选（hand-ai 维护者裁量）

HandBox 不掌握下列模型的 effort 限制权威信息，列在这里供 hand-ai 维护者补充或否决：

- `openai/o1-pro`：传闻只支持 high
- `openai/o1`：minimal/low/medium/high 全支持？
- `openai/o3-mini`、`o3`：?
- `openai/gpt-5`、`gpt-5-mini`、`gpt-5-nano`、`gpt-5-codex`、`gpt-5.1-codex-*`、`gpt-5.2-*`：?

这些不在本 PR 强制范围。**P1 PR 的 scope 建议是 `gpt-5-pro` 一个模型 + 4 个镜像**，把 schema 实践落地；其他模型后续补丁。

### 影响

- 下游 embedder 调 `get_supported_thinking_levels(model)` 即得到正确的 effort 列表。
- 已有 wire-payload 行为不变（除 `xhigh: null` 显式 mark 之外，hand-ai 内部 effort 路由不读这些 key 做行为分支）。
- 加测试：`gpt-5-pro` 的 `get_supported_thinking_levels` 只返回 `[None, Some(High)]`。

### 估算

10-30 行 JSON 修改 + 一个 sanity test。**0.5 天**。

---

## 4. P2：`Model.thinking_token_budget` 新字段

### 现状

Gemini 2.5 系列模型的 reasoning 是 **token-budget driven**（不是 effort-driven）。Google API 接受：

- `thinking_budget = -1` → dynamic（让模型自己决定）
- `thinking_budget = 0` → disabled（关闭推理；只 flash/flash-lite 支持）
- `thinking_budget = N` → 固定 N tokens

每个模型有不同的合法 `N` 区间：

| Model | Min | Max | Supports Dynamic | Supports Disabled |
|---|---|---|---|---|
| `gemini-2.5-pro` | 128 | 32768 | ✓ | ✗ |
| `gemini-2.5-flash` | 1 | 24576 | ✓ | ✓ |
| `gemini-2.5-flash-lite` | 512 | 24576 | ✓ | ✓ |

HandBox `llm_config.json` 是这份数据**当下唯一的来源**。hand-ai catalog 里这 3 个 gemini 模型只有 `reasoning: true`，没有任何 budget 信息。

`ThinkingBudgets`（types.rs:533-539）描述的是 per-effort-level 的**固定** budget，不是 user-pickable range。两种语义不能复用同一个结构。

### 提案 API

在 `Model` 上加一个新字段：

```rust
/// User-pickable token-budget range for reasoning, when the API uses
/// numeric budgets (e.g. Google Generative AI's `thinking_budget`).
/// None means the model uses effort-based reasoning (or no reasoning).
#[serde(skip_serializing_if = "Option::is_none", rename = "thinkingTokenBudget")]
pub thinking_token_budget: Option<TokenBudgetRange>,
```

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenBudgetRange {
    /// Inclusive lower bound of the user-pickable budget (in tokens).
    pub min: u32,
    /// Inclusive upper bound of the user-pickable budget (in tokens).
    pub max: u32,
    /// True if the API accepts `-1` to mean "let the model decide".
    pub supports_dynamic: bool,
    /// True if the API accepts `0` to mean "disable reasoning".
    pub supports_disabled: bool,
}
```

### 数据填充

按上表，给 hand-ai catalog 中所有 gemini-2.5 变体（约 30 条，跨 google / google-gemini-cli / google-vertex / openrouter / vercel-ai-gateway / github-copilot 等 provider）补 `thinkingTokenBudget`。

注：cross-provider 镜像（如 `openrouter/google/gemini-2.5-pro`、`vercel-ai-gateway/google/gemini-2.5-pro`）需要单独 verify——OpenRouter 上 Google 模型可能改用 effort 暴露，那种情况就**不**该填 `thinkingTokenBudget`（保持 None，走 effort 路径）。

### 影响

- **Backwards-compatible 序列化**：新字段 `#[serde(skip_serializing_if = "Option::is_none")]`，旧 catalog 反序列化时该字段为 `None`，行为不变。
- 新结构 `#[non_exhaustive]`，与 hand-ai 现有 forward-compat 约定一致。
- 下游 embedder 通过 `model.thinking_token_budget.as_ref().map(|b| b.min..=b.max)` 即可驱动 UI。

### 估算

- ~50 行新代码（types.rs 字段 + 文档 + 测试）
- ~30 条 JSON 模型修改
- 1 个集成测试（`Model { id: "gemini-2.5-pro", ... }.thinking_token_budget` deserialize 正确）

**1-1.5 天。**

---

## 5. P3：`ApiCapabilities.reasoning_summary_modes` 新字段

### 现状

OpenAI Responses API 接受 `reasoning.summary` 字段，取值 `auto` / `detailed` / `concise`（某些 preview 模型）。HandBox `llm_config.json` 当下声明：

- 默认（common）：`["auto", "detailed"]`
- `openai/computer-use-preview` 例外：`["auto", "concise", "detailed"]`

hand-ai 当前完全没有 summary 概念。

### 提案 API

在 `ApiCapabilities` 上加一个字段：

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiCapabilities {
    pub tools: bool,
    /// Summary modes this API protocol exposes for reasoning responses.
    /// Empty slice = the API has no summary surface.
    pub reasoning_summary_modes: &'static [SummaryMode],
}
```

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SummaryMode {
    Auto,
    Concise,
    Detailed,
}
```

### `Api::capabilities()` 表

| API | reasoning_summary_modes |
|---|---|
| `OpenAIResponses` / `AzureOpenAiResponses` / `OpenAICodexResponses` | `&[Auto, Detailed]` |
| 其他全部 | `&[]` |

### 关于 `computer-use-preview` 的 `concise`

该模型不在 hand-ai catalog 中（grep 验证）。HandBox 这条 exception 是给一个 hand-ai 不知道的模型。两个处理路径：

1. 若 hand-ai 决定 catalog 不收录 `computer-use-preview`，HandBox 保留该 exception（这份 metadata 留在 HandBox 端）。
2. 若收录，则在 `Model` 上加 optional 字段 `summary_modes_override: Option<&'static [SummaryMode]>` 让 catalog 表达 per-model 扩展。

P3 PR 建议**只做路径 1**：API-level capability 覆盖 95% 场景，per-model override 留 followup。

### 估算

- ~30 行新代码（capabilities.rs 字段 + Api::capabilities() match arms + tests）
- 0 行 JSON 修改（API-level，不需要数据文件改）

**0.5 天。**

---

## 6. P4（defer）：OpenRouter cross-protocol reasoning

### 现象

HandBox `llm_config.json` 有这一条：

```json
"openrouter.parameters.reasoning": {
  "default_props": ["effect", "exclude"],
  "special_props": {
    "^anthropic/.*$": ["max_tokens"]
  }
}
```

含义：OpenRouter 上 `anthropic/*` 前缀的模型（比如 `anthropic/claude-opus-4`），需要额外暴露 `max_tokens` 参数——因为底层走 Anthropic 协议的 token-budget reasoning，而其他 OpenRouter 模型走 OpenAI 协议的 effort reasoning。

但 hand-ai catalog 把所有 OpenRouter 模型都标 `api: openai-completions`，看不出底层 vendor 协议差异。

### 为何 defer

这是 model-level 的 reasoning protocol 维度，需要新枚举：

```rust
pub enum ReasoningProtocol {
    /// Discrete effort levels (OpenAI Responses, Google direct)
    Effort,
    /// Numeric token budget (Google direct, Anthropic via OpenRouter)
    TokenBudget,
    /// Both surfaces (some Anthropic models)
    EffortAndTokenBudget,
}
```

加上 `Model.reasoning_protocol: Option<ReasoningProtocol>` 字段。

但：

- 这个维度只为 OpenRouter 这一个 aggregator 服务，scope 跟其他 P1-P3 不对称
- 对 native Anthropic 模型同样有效，但 hand-ai 现在没有暴露这个区分
- 如果未来 OpenRouter 加更多 cross-protocol 路由（比如 google/* 走 Google native），数据维度还会膨胀

建议：先把 P1-P3 落地，HandBox 端保留 `special_props` 这一行 7 字符的正则，观察 1-2 季度后再决定是否值得做 P4。

---

## 7. 总体影响与时间表

| PR | Type | LOC | 估算 |
|---|---|---|---|
| P1 (thinking_level_map for gpt-5-pro) | data-only | ~30 | 0.5 天 |
| P2 (thinking_token_budget for gemini-2.5) | API + data | ~80 | 1-1.5 天 |
| P3 (ApiCapabilities.reasoning_summary_modes) | API + tests | ~30 | 0.5 天 |
| **Total** | | ~140 | ~2-3 天 |

P1-P3 之间**无依赖**，可以并行做。HandBox 这边在 hand-ai 发布带这些 metadata 的版本后，会立刻删除 `llm_config.json` 里对应字段（预估 -60 行 JSON）。

## 8. 不在范围内

- **UI 渲染元数据**（slider/switch、step、show_toggle、Chinese tooltips）— 永远是 HandBox 产品域，不进 hand-ai。
- **`turn_count`**（聊天历史轮数）— HandBox 会话概念，跟 LLM 协议无关。
- **provider 图标/显示名**（`/logo-openai.png`、"OpenAI" 大小写）— HandBox 资产。
- **custom-provider 模板**（onboarding 起步配置）— HandBox onboarding 流程。

## 9. 验证策略

每个 PR 落地时建议附带的测试：

- **P1**：`get_supported_thinking_levels(gpt_5_pro_model)` 返回 `vec![None, Some(High)]`。
- **P2**：`Model { id: "gemini-2.5-pro", ... }.thinking_token_budget == Some(TokenBudgetRange { min: 128, max: 32768, supports_dynamic: true, supports_disabled: false })`；针对每个 gemini-2.5-* 变体 spot-check。
- **P3**：`Api::OpenAIResponses.capabilities().reasoning_summary_modes == &[SummaryMode::Auto, SummaryMode::Detailed]`；非 Responses API 返回 `&[]`。
- **HandBox 集成**：HandBox 升级 hand-ai 后跑 UT-DISSOLVE-001..004（参考 `docs/user-tests/dissolve-handbox-llm.md`）。

## 10. 联系信息

- HandBox 侧：HandBox repo `feat/hand-ai-integration` 分支，HEAD `9eb6b9c`（2026-05-27）。
- 数据审计原始命令保存在 `/tmp/audit_reasoning.py` / `/tmp/audit_gemini.py` / `/tmp/audit_gpt5.py`，hand-ai 维护者可复用验证 catalog 现状。
- 本文档由 HandBox 侧 Claude 起草；hand-ai 侧维护者如对 API shape 有不同意见，欢迎在该文档上直接讨论或拆 PR 自行裁量。
