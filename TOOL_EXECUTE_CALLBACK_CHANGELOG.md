# 工具执行回调重构说明

## 变更概述

重构了 `execute_tool_calls` 方法的工具执行回调机制，从两个独立的回调简化为一个统一的回调，使用 `status` 参数区分执行状态。

## 主要变更

### 1. 新增 `ToolExecuteStatus` 枚举

**位置:** `src-tauri/src/services/message.rs:68-76`

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolExecuteStatus {
    /// 工具执行中
    Executing,
    /// 工具执行完成
    Finished,
}
```

### 2. 新增 `ToolExecuteCallback` trait

**位置:** `src-tauri/src/services/message.rs:78-85`

```rust
/// 工具执行回调：当工具执行状态变化时调用
///
/// 参数:
/// - `message_id`: 消息的唯一标识符
/// - `tool_call_ids`: 工具调用ID列表
/// - `status`: 工具执行状态
pub trait ToolExecuteCallback: FnMut(String, Vec<String>, ToolExecuteStatus) + Send + 'static {}
```

**回调参数:**
1. `message_id: String` - 消息 ID（不是 stream_id）
2. `tool_call_ids: Vec<String>` - 工具调用 ID 列表
3. `status: ToolExecuteStatus` - 执行状态（Executing 或 Finished）

### 3. 更新 `execute_tool_calls` 方法签名

**位置:** `src-tauri/src/services/message.rs:1115-1125`

**修改前:**
```rust
pub async fn execute_tool_calls(
    &self,
    message_id: String,
    tool_call_ids: Vec<String>,
    start_callback: impl StreamStartCallback,
    streaming_callback: impl StreamingCallback,
    end_callback: impl StreamEndCallback,
    error_callback: impl StreamErrorCallback,
    tool_executing_callback: impl ToolExecutingCallback,  // ❌ 移除
    tool_finished_callback: impl ToolFinishedCallback,    // ❌ 移除
)
```

**修改后:**
```rust
pub async fn execute_tool_calls(
    &self,
    message_id: String,
    tool_call_ids: Vec<String>,
    start_callback: impl StreamStartCallback,
    streaming_callback: impl StreamingCallback,
    end_callback: impl StreamEndCallback,
    error_callback: impl StreamErrorCallback,
    tool_execute_callback: impl ToolExecuteCallback,     // ✅ 新增
)
```

### 4. 触发回调时机

**位置:** `src-tauri/src/services/message.rs`

```rust
// 执行前触发 Executing 状态
tool_execute_callback(
    message_id.clone(),
    tool_call_ids.clone(),
    ToolExecuteStatus::Executing,
);

// ... 执行所有工具调用 ...

// 执行后触发 Finished 状态
tool_execute_callback(
    message_id.clone(),
    tool_call_ids.clone(),
    ToolExecuteStatus::Finished,
);
```

### 5. 前端事件更新

**事件名称:** `tool_execute`

**事件数据:**
```typescript
{
  messageId: string;       // 消息 ID
  toolCallIds: string[];   // 工具调用 ID 数组
  status: "executing" | "finished";
}
```

## 优势

1. **简化接口**: 从两个回调简化为一个，参数更清晰
2. **语义化状态**: 使用枚举类型代替字符串，类型安全
3. **统一管理**: 前端基于 `messageId` 统一管理工具执行状态
4. **易于扩展**: 如需添加新状态（如 `Failed`），只需扩展枚举即可

## 前端迁移指南

### 旧版本（已移除）

```typescript
// ❌ 旧版本：监听两个独立事件
await listen('tool_executing', (event) => {
  const { streamId, toolCallId, toolName } = event.payload;
  // ...
});

await listen('tool_finished', (event) => {
  const { streamId, toolCallId, toolName, result } = event.payload;
  // ...
});
```

### 新版本（推荐）

```typescript
// ✅ 新版本：监听单一事件，根据 status 区分
await listen('tool_execute', (event) => {
  const { messageId, toolCallIds, status } = event.payload;

  if (status === 'executing') {
    // 工具开始执行
    toolStatuses[messageId] = 'executing';
  } else if (status === 'finished') {
    // 工具执行完成
    toolStatuses[messageId] = 'idle';
  }
});
```

## 相关文件

- **后端实现:** `src-tauri/src/services/message.rs`
- **命令层:** `src-tauri/src/commands/message.rs`
- **服务导出:** `src-tauri/src/services/mod.rs`
- **使用文档:** `TOOL_EXECUTION_EVENTS.md`

## 测试建议

1. 测试单个工具调用的执行流程
2. 测试多个工具调用的执行流程
3. 测试工具执行失败的情况
4. 验证前端事件监听和状态更新
5. 检查按钮禁用/启用状态切换

---

**更新日期:** 2025-10-03
