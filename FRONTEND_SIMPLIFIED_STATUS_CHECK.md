# 前端工具执行状态检查简化说明

## 问题

之前的实现中，UI 组件使用了复杂的逻辑来判断工具是否在执行：

```typescript
// ❌ 旧实现：过于复杂
function isAnyToolCallExecuting(): boolean {
  if (!message?.id) return false;
  
  const calls = toolCalls();
  return calls.some(call => {
    if (call.id && message.id) {
      return messageStore.isToolCallExecuting(message.id, call.id);
    }
    return false;
  });
}
```

**问题：**
1. 逻辑复杂，需要遍历所有工具调用
2. 依赖 `toolCallId`，但后端已经基于 `messageId` 管理状态
3. 不必要的性能开销
4. 与后端状态管理不一致

## 解决方案

直接使用后端 `tool_execute` 事件提供的状态，基于 `messageId` 判断：

```typescript
// ✅ 新实现：简洁直观
const isExecuting = $derived(() => {
  if (!message?.id) return false;
  return messageStore.isToolCallExecuting(message.id);
});
```

## 核心改动

### 1. UI 组件 (`AssistantMessageView.svelte`)

**移除复杂函数:**
```diff
- function isAnyToolCallExecuting(): boolean {
-   if (!message?.id) return false;
-   const calls = toolCalls();
-   return calls.some(call => {
-     if (call.id && message.id) {
-       return messageStore.isToolCallExecuting(message.id, call.id);
-     }
-     return false;
-   });
- }
```

**使用 $derived 响应式计算:**
```diff
+ const isExecuting = $derived(() => {
+   if (!message?.id) return false;
+   return messageStore.isToolCallExecuting(message.id);
+ });
```

**更新按钮:**
```diff
  <button
    onclick={handleExecuteToolCalls}
-   disabled={toolCalls().length === 0 || isAnyToolCallExecuting()}
+   disabled={toolCalls().length === 0 || isExecuting()}
  >
-   {#if isAnyToolCallExecuting()}
+   {#if isExecuting()}
      执行中...
    {:else}
      执行工具调用
    {/if}
  </button>
```

### 2. MessageStore (`message.svelte.ts`)

**简化方法签名:**
```diff
- isToolCallExecuting(messageId: string, toolCallId?: string): boolean {
+ isToolCallExecuting(messageId: string): boolean {
    return this.state.executingMessages.has(messageId);
  }
```

## 工作原理

### 状态流

```
后端触发 tool_execute { messageId: "msg-123", status: "executing" }
  ↓
messageStore.state.executingMessages.add("msg-123")
  ↓
isExecuting() 自动重新计算 (Svelte 5 响应式)
  ↓
返回 true
  ↓
UI 自动更新：按钮显示"执行中..."
```

### 为什么更简单？

1. **直接状态映射**: `messageId` → `executing/idle`
2. **响应式计算**: Svelte 5 的 `$derived` 自动追踪依赖
3. **O(1) 查询**: Set 数据结构，快速查询
4. **无需遍历**: 不需要检查每个工具调用

## 对比

| 特性 | 旧实现 | 新实现 |
|------|--------|--------|
| 逻辑复杂度 | 高（遍历+判断） | 低（直接查询） |
| 代码行数 | ~10 行 | ~3 行 |
| 性能 | O(n) | O(1) |
| 类型安全 | 未使用的参数警告 | 完全类型安全 |
| 响应式 | 函数调用 | $derived 自动 |
| 可维护性 | 难 | 易 |

## 使用示例

在任何需要检查工具执行状态的地方：

```svelte
<script lang="ts">
  import { messageStore } from '$lib/states';
  
  // 简单的响应式计算
  const isExecuting = $derived(() => {
    return messageStore.isToolCallExecuting(message.id);
  });
</script>

<!-- 按钮自动响应状态变化 -->
<button disabled={isExecuting()}>
  {isExecuting() ? '执行中...' : '执行工具'}
</button>
```

## 优势

✅ **简洁**: 3 行代码 vs 10+ 行  
✅ **高效**: O(1) 查询 vs O(n) 遍历  
✅ **准确**: 直接使用后端状态，无需推断  
✅ **响应式**: Svelte 5 自动更新，无需手动触发  
✅ **类型安全**: 移除未使用的参数，无警告  

## 测试验证

### 手动测试

1. 点击"执行工具调用"按钮
2. 观察按钮立即变为"执行中..."
3. 工具执行完成后，按钮恢复"执行工具调用"

### 控制台验证

```javascript
// 检查状态
messageStore.state.executingMessages

// 手动设置状态测试 UI
messageStore.state.executingMessages.add('msg-123')
messageStore.state.executingMessages.delete('msg-123')
```

## 总结

通过这次简化，前端代码更加清晰、高效，完全依赖后端提供的准确状态。这是正确的架构设计：

- **后端**: 管理状态，通过事件通知前端
- **前端**: 消费状态，响应式更新 UI

---

**更新日期**: 2025-10-03
