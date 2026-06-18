渲染信封协议（Render Envelope Protocol）
------

本文件约定 Agent 输出消息中「动态渲染信封」的格式契约。Agent 在消息正文里嵌入一个 JSON
信封，前端据此选择对应的渲染器（renderer）把结构化结果画成卡片，而不是把原始 JSON 逐字符渲染出来。

本协议与解析器/校验器一一对应：

- 解析器：`src/lib/components/chat/renderers/envelope.ts`（`parseEnvelope`）
- translation 校验器：`src/lib/components/chat/renderers/translation.ts`（`validateTranslation`）
- 共享类型：`src/lib/components/chat/renderers/types.ts`

下文写到的字段名、判别值、载体识别规则，都是上述代码当前接受的结构。文档与代码若有出入，
`docs-envelope.test.ts` 会因「文档示例信封无法被解析/校验命中」而失败。

## 1. `__render` 判别字段

信封是一个顶层 JSON 对象，形如：

```json
{ "__render": "<type>", "data": { } }
```

- `__render`：**必需**，**非空字符串**（trim 后长度 > 0）。它是判别字段（discriminator），
  用于在注册表中选择渲染器。
- `data`：信封载荷，原样透传给被选中的渲染器；解析器**不**校验 `data` 的形状，形状校验由各
  渲染器自己负责。
- 其余顶层字段（如 `version`、`id`）被忽略，不影响解析。

`__render` 的取值（type）**大小写敏感、精确匹配**：`translation` 命中，`Translation`、
`TRANSLATION` 都不命中。

## 2. 载体识别规则

只有当**整条消息**恰好是下面两种载体之一时，才被识别为信封；否则按普通文本/Markdown 渲染。

1. **裸 JSON**：消息 trim 之后，整体就是一个 JSON 对象（首字符 `{`、尾字符 `}`）。对象前后
   有任何非空白文本都会使其失格。
2. **单个 ` ```json ` 代码块**：消息 trim 之后，整体恰好是一个围栏代码块，其语言标记为 `json`
   （大小写不敏感）。出现以下任一情况即失格：围栏外有正文、有多个围栏、语言不是 `json`
   （如 `json5`/`jsonc`/无语言）、围栏未闭合。

两种载体共用同一结构契约（见第 1 节）：解析结果必须是带非空字符串 `__render` 的纯对象。
任何畸形、空、或非信封输入都解析为 `null`（解析器不抛异常）。

解析成功后得到 `{ type, data }`：`type` 即 `__render` 的值，`data` 即载荷原样。

## 3. translation 的 data schema

`translation` 渲染器的 `data` 形状如下（字段名大小写敏感）：

| 字段          | 是否必需 | 类型   | 说明                                   |
| ------------- | -------- | ------ | -------------------------------------- |
| `term`        | 可选     | string | 原词 / 原短语                          |
| `translation` | **必需** | string | 译文；必须是 **trim 后非空** 的字符串  |
| `phonetic`    | 可选     | string | 音标                                   |
| `explanation` | 可选     | string | 释义 / 例句等补充说明                  |

校验规则（对应 `validateTranslation`）：

- `data` 必须是纯对象（非对象、`null`、数组、基本类型一律被拒，返回 `null`）。
- `translation` 必需且为 trim 后非空字符串，否则整条载荷被拒。
- `term`、`phonetic`、`explanation` 为可选：缺省可以；存在但**非字符串**时，该字段被**丢弃**
  （从结果对象中省略），而不会导致整条载荷失败。
- 额外字段被忽略。

## 4. 注册表扩展约定

新增一种渲染能力的成本是局部且固定的：

> **新增一个渲染器 = 在 `src/lib/components/chat/renderers/` 下加一个 renderer
> （`type` + `validate` + `component`），并把它注册到注册表（`rendererRegistry.register(...)`），
> 无需改动 `MessageAssistant` 主链路。**

主链路只做三件事：用 `parseEnvelope` 识别信封 → 按 `type` 在注册表里查渲染器 →
用渲染器的 `validate` 校验 `data` 并交给 `component` 绘制。因此渲染器之间彼此独立，新增/调整
互不影响，也不触碰消息渲染主流程。

## 5. 可粘贴进 Agent system prompt 的模板

把下面这段（中括号内为你按场景替换的指引）粘贴进 Agent 的 system prompt，即可让模型把翻译
结果输出为可被前端识别的 translation 信封：

> When the user's request is a translation task, output **only** a single render
> envelope and nothing else — either as bare JSON, or wrapped in exactly one
> ` ```json ` fenced code block. Do not add any prose before or after it.
>
> The envelope MUST be:
>
> ```json
> { "__render": "translation", "data": { } }
> ```
>
> where `data` follows this schema (field names are case-sensitive):
>
> - `term` (optional, string): the original word/phrase.
> - `translation` (required, non-empty string): the translated text.
> - `phonetic` (optional, string): pronunciation.
> - `explanation` (optional, string): extra notes / example sentences.
>
> Use the exact discriminator value `translation` (lowercase). Omit optional
> fields you have no value for rather than sending empty strings.

### 示例信封

下面是一个完整、合法的 translation 信封（`docs-envelope.test.ts` 会从本节抽取此代码块作为
fixture，喂给 `parseEnvelope` + `validateTranslation` 做一致性校验）：

```json
{
  "__render": "translation",
  "data": {
    "term": "serendipity",
    "translation": "意外发现美好事物的能力",
    "phonetic": "/ˌserənˈdɪpɪti/",
    "explanation": "指在无意间发现珍贵或令人愉快事物的运气与才能。"
  }
}
```
