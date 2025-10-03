# 工具执行状态调试指南

## 问题排查

如果点击"执行工具调用"按钮后，按钮状态没有变化（没有显示"执行中..."），请按照以下步骤排查：

## 调试步骤

### 1. 打开浏览器开发者工具

按 `F12` 或右键 → 检查，打开开发者工具的 Console 标签。

### 2. 点击"执行工具调用"按钮

观察控制台输出，应该看到以下日志序列：

```
执行工具调用: [...]                                     // 来自组件
[executeAllToolCalls] 开始: { messageId: "...", ... }   // messageStore
[executeAllToolCalls] 提取的工具调用IDs: [...]          // messageStore
[executeAllToolCalls] 调用 executeToolCalls...          // messageStore
[executeToolCalls] 开始执行工具调用: { ... }            // messageStore
[executeToolCalls] 设置事件监听器...                     // messageStore
[listenToStreamEvents] 注册 tool_execute 事件监听器     // API 层
[executeToolCalls] 事件监听器设置完成，调用后端API...   // messageStore
[executeToolCalls] 后端API调用完成                      // messageStore
```

### 3. 检查后端事件

如果前端日志正常，接下来应该看到后端发送的事件：

```
[tool_execute 事件] 收到事件: {                         // API 层
  messageId: "msg-123",
  toolCallIds: ["tool-1", "tool-2"],
  status: "executing"
}
[executeToolCalls] 工具执行状态变化: { ... }            // messageStore
[executeToolCalls] 添加执行状态: msg-123                // messageStore
[executeToolCalls] 添加后 executingMessages: ["msg-123"] // messageStore
isExecuting 计算中... msg-123 true                      // 组件
```

### 4. 检查 UI 响应

观察 `isExecuting` 的计算日志：

```
isExecuting 计算中... msg-123 true    // 应该显示 true
[isToolCallExecuting] 检查: {          // messageStore 方法
  messageId: "msg-123",
  result: true,
  executingMessages: ["msg-123"]
}
```

## 常见问题

### 问题 1: 没有看到 `[listenToStreamEvents]` 日志

**原因**: 事件监听器没有注册  
**检查**: 
- `handlers.onToolExecute` 是否正确传递
- `listenToStreamEvents` 的调用是否正确

**解决**: 检查 `messageStore.executeToolCalls` 中的事件监听器设置

### 问题 2: 没有收到 `tool_execute` 事件

**原因**: 后端没有发送事件  
**检查**:
1. 后端 Rust 代码是否正确触发回调
2. 事件名称是否匹配（应该是 `tool_execute`）

**验证后端**:
```bash
# 查看后端日志
cargo run
# 应该看到类似的日志：
# [tool_execute] Tool execution status changed to Executing
```

### 问题 3: 事件收到但状态没更新

**原因**: 状态更新逻辑有问题  
**检查**:
```
[executeToolCalls] 添加执行状态: msg-123
[executeToolCalls] 添加后 executingMessages: []  // ❌ 应该是 ["msg-123"]
```

**解决**: Set 的 add 方法可能没有触发响应式更新

### 问题 4: 状态更新但 UI 不响应

**原因**: Svelte 5 响应式未正确设置  
**检查**:
- `$derived.by()` 是否正确使用
- `isExecuting` 是否是值而不是函数
- 按钮中是否使用 `isExecuting` 而不是 `isExecuting()`

**正确用法**:
```svelte
<script>
  // ✅ 正确
  const isExecuting = $derived.by(() => {
    return messageStore.isToolCallExecuting(message.id);
  });
</script>

<!-- ✅ 正确 -->
<button disabled={isExecuting}>
  {#if isExecuting}
    执行中...
  {/if}
</button>
```

## 手动测试状态

在浏览器控制台中手动测试状态系统：

```javascript
// 1. 检查 messageStore 是否可访问
window.messageStore = messageStore;  // 在代码中临时添加

// 2. 手动添加执行状态
messageStore.state.executingMessages.add('test-msg-id');

// 3. 检查 UI 是否响应（按钮应该显示"执行中..."）

// 4. 手动移除执行状态
messageStore.state.executingMessages.delete('test-msg-id');

// 5. 检查 UI 是否恢复（按钮应该恢复正常）
```

## 完整事件流程图

```
用户点击按钮
  ↓
handleExecuteToolCalls()
  ↓
messageStore.executeAllToolCalls(messageId, toolCalls)
  ↓
提取 toolCallIds
  ↓
messageStore.executeToolCalls(messageId, toolCallIds)
  ↓
设置事件监听器 (onToolExecute)
  ↓
调用后端 API (executeToolCallsStream)
  ↓
【等待后端响应】
  ↓
后端发送 tool_execute 事件 (status: executing)
  ↓
onToolExecute 回调触发
  ↓
executingMessages.add(messageId)
  ↓
isExecuting 重新计算 (Svelte 5 响应式)
  ↓
UI 更新：按钮显示"执行中..."
  ↓
【后端执行 MCP 工具】
  ↓
后端发送 tool_execute 事件 (status: finished)
  ↓
onToolExecute 回调触发
  ↓
executingMessages.delete(messageId)
  ↓
isExecuting 重新计算
  ↓
UI 更新：按钮恢复正常
```

## 预期日志输出

### 完整的控制台输出示例

```
// === 用户点击按钮 ===
执行工具调用: [{id: "tool-1", function: {...}}, ...]

// === 前端准备 ===
[executeAllToolCalls] 开始: {messageId: "msg-123", toolCallsCount: 2}
[executeAllToolCalls] 提取的工具调用IDs: ["tool-1", "tool-2"]
[executeAllToolCalls] 调用 executeToolCalls...
[executeToolCalls] 开始执行工具调用: {messageId: "msg-123", toolCallIds: ["tool-1", "tool-2"]}
[executeToolCalls] 设置事件监听器...
[listenToStreamEvents] 注册 tool_execute 事件监听器
[executeToolCalls] 事件监听器设置完成，调用后端API...
[executeToolCalls] 后端API调用完成

// === 后端开始执行 ===
[tool_execute 事件] 收到事件: {messageId: "msg-123", toolCallIds: ["tool-1", "tool-2"], status: "executing"}
[executeToolCalls] 工具执行状态变化: {messageId: "msg-123", toolCallIds: [...], status: "executing"}
[executeToolCalls] 当前 executingMessages: []
[executeToolCalls] 添加执行状态: msg-123
[executeToolCalls] 添加后 executingMessages: ["msg-123"]

// === UI 响应 ===
isExecuting 计算中... msg-123 true
[isToolCallExecuting] 检查: {messageId: "msg-123", result: true, executingMessages: ["msg-123"]}

// === 后端完成执行 ===
[tool_execute 事件] 收到事件: {messageId: "msg-123", toolCallIds: ["tool-1", "tool-2"], status: "finished"}
[executeToolCalls] 工具执行状态变化: {messageId: "msg-123", toolCallIds: [...], status: "finished"}
[executeToolCalls] 当前 executingMessages: ["msg-123"]
[executeToolCalls] 移除执行状态: msg-123
[executeToolCalls] 移除后 executingMessages: []

// === UI 恢复 ===
isExecuting 计算中... msg-123 false
[isToolCallExecuting] 检查: {messageId: "msg-123", result: false, executingMessages: []}
```

## 清理调试日志

测试完成后，可以移除以下日志：

1. `AssistantMessageView.svelte:89` - `console.log('isExecuting 计算中...')`
2. `message.svelte.ts:88` - `console.log('[isToolCallExecuting] 检查:')`
3. `message.svelte.ts:523-572` - 所有 `[executeToolCalls]` 日志
4. `message.svelte.ts:597-617` - 所有 `[executeAllToolCalls]` 日志
5. `message.ts:123-131` - 所有 `[listenToStreamEvents]` 和 `[tool_execute 事件]` 日志

---

**更新日期**: 2025-10-03
