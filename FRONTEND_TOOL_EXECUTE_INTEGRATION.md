# 前端工具执行状态集成指南

## 概述

本文档说明如何在前端使用 `tool_execute` 事件来实时显示 MCP 工具的执行状态。

## 架构流程

```
用户点击"执行工具" 
  ↓
AssistantMessageView.svelte (调用 messageStore.executeAllToolCalls)
  ↓
messageStore.executeToolCalls (设置事件监听)
  ↓
后端发送 tool_execute 事件 (status: executing)
  ↓
messageStore 更新 executingMessages (添加 messageId)
  ↓
AssistantMessageView 响应式更新 UI (显示 "执行中...")
  ↓
后端执行完所有工具
  ↓
后端发送 tool_execute 事件 (status: finished)
  ↓
messageStore 更新 executingMessages (移除 messageId)
  ↓
AssistantMessageView 响应式更新 UI (恢复 "执行工具调用")
```

## 已实现的功能

### 1. API 层 (`src/lib/api/message.ts`)

**新增接口:**
```typescript
export interface StreamEventHandlers {
  onStart?: (data: { streamId: string; messageId: string }) => void;
  onChunk?: (data: { ... }) => void;
  onEnd?: (data: { ... }) => void;
  onError?: (error: any) => void;
  onToolExecute?: (data: { 
    messageId: string; 
    toolCallIds: string[]; 
    status: 'executing' | 'finished' 
  }) => void;  // ✅ 新增
}
```

**更新监听函数:**
```typescript
export async function listenToStreamEvents(
  handlers: StreamEventHandlers, 
  eventPrefix: string = 'message_stream'
) {
  const listeners = [
    // ... 其他事件监听器
  ];

  // 如果提供了 onToolExecute 处理器，添加工具执行事件监听
  if (handlers.onToolExecute) {
    listeners.push(
      listen('tool_execute', (event) => {
        handlers.onToolExecute?.(event.payload as any);
      })
    );
  }

  const unlisten = await Promise.all(listeners);
  return () => unlisten.forEach(fn => fn());
}
```

### 2. 状态管理层 (`src/lib/states/message.svelte.ts`)

**状态定义更新:**
```typescript
interface MessageState {
  // ... 其他状态
  // 工具执行状态 - 记录哪些消息正在执行工具
  executingMessages: Set<string>; // ✅ 改为基于 messageId
}
```

**方法更新:**
```typescript
// 检查消息是否正在执行工具
isToolCallExecuting(messageId: string, toolCallId?: string): boolean {
  return this.state.executingMessages.has(messageId);
}

async executeToolCalls(messageId: string, toolCallIds: string[]): Promise<void> {
  // 监听工具执行流式事件
  this.currentStreamUnlisten = await listenToStreamEvents(
    {
      ...this.createStreamEventHandlers(),
      // 添加工具执行状态回调
      onToolExecute: (data) => {
        console.log('工具执行状态变化:', data);
        if (data.status === 'executing') {
          // 设置执行状态
          this.state.executingMessages.add(data.messageId);
        } else if (data.status === 'finished') {
          // 清除执行状态
          this.state.executingMessages.delete(data.messageId);
        }
      }
    },
    'tool_execute_stream'
  );

  await messageApi.executeToolCallsStream(messageId, toolCallIds);
}
```

### 3. UI 组件 (`src/lib/components/chat/views/AssistantMessageView.svelte`)

**现有实现已就绪:**
```svelte
<script lang="ts">
  // 检查是否有任何工具调用正在执行
  function isAnyToolCallExecuting(): boolean {
    if (!message?.id) return false;
    
    const calls = toolCalls();
    return calls.some(call => {
      if (call.id && message.id) {
        // messageStore.isToolCallExecuting 现在基于 messageId 检查
        return messageStore.isToolCallExecuting(message.id, call.id);
      }
      return false;
    });
  }
</script>

<!-- 工具调用按钮 -->
<button
  class="px-2 py-1 text-xs bg-blue-600 hover:bg-blue-700 text-white rounded disabled:opacity-50"
  onclick={handleExecuteToolCalls}
  disabled={toolCalls().length === 0 || isAnyToolCallExecuting()}
>
  {#if isAnyToolCallExecuting()}
    <div class="flex items-center gap-1">
      <div class="w-3 h-3 border border-white border-t-transparent rounded-full animate-spin"></div>
      执行中...
    </div>
  {:else}
    执行工具调用
  {/if}
</button>
```

## 工作原理

### 状态流转

1. **初始状态**: `executingMessages = Set()`
2. **点击执行**: 调用 `messageStore.executeAllToolCalls(messageId, toolCalls)`
3. **开始执行**: 
   - 后端触发 `tool_execute` 事件 `{ messageId, toolCallIds, status: 'executing' }`
   - messageStore 添加 `messageId` 到 `executingMessages`
   - `isAnyToolCallExecuting()` 返回 `true`
   - 按钮显示"执行中..."并禁用
4. **执行完成**:
   - 后端触发 `tool_execute` 事件 `{ messageId, toolCallIds, status: 'finished' }`
   - messageStore 从 `executingMessages` 删除 `messageId`
   - `isAnyToolCallExecuting()` 返回 `false`
   - 按钮恢复"执行工具调用"并启用

### 响应式更新

由于使用了 Svelte 5 的 `$state` 和 `$derived`，状态变化会自动触发 UI 更新：

```typescript
// messageStore 中的状态是响应式的
private state = $state<MessageState>({
  executingMessages: new Set(),
  // ...
});

// 组件中的检查函数会自动响应状态变化
function isAnyToolCallExecuting(): boolean {
  // 读取 messageStore.isToolCallExecuting
  // 该方法读取 state.executingMessages
  // 状态变化 → 自动重新计算 → UI 自动更新
}
```

## 调试技巧

### 1. 查看事件日志

在浏览器控制台中可以看到：

```
工具执行状态变化: { 
  messageId: "msg-123", 
  toolCallIds: ["tool-1", "tool-2"], 
  status: "executing" 
}

工具执行状态变化: { 
  messageId: "msg-123", 
  toolCallIds: ["tool-1", "tool-2"], 
  status: "finished" 
}
```

### 2. 检查状态

在浏览器控制台中：

```javascript
// 查看当前正在执行的消息
messageStore.state.executingMessages

// 检查特定消息是否在执行
messageStore.isToolCallExecuting('msg-123')
```

### 3. 模拟测试

可以手动触发状态变化来测试 UI：

```javascript
// 模拟开始执行
messageStore.state.executingMessages.add('msg-123');

// 模拟执行完成
messageStore.state.executingMessages.delete('msg-123');
```

## 注意事项

1. **messageId 为 key**: 现在基于 `messageId` 而非 `toolCallId` 管理状态，更简洁高效
2. **自动清理**: 错误时也会清除执行状态，避免状态泄漏
3. **响应式**: 使用 Svelte 5 的响应式系统，无需手动触发更新
4. **事件监听**: 监听器会在流程完成或出错时自动清理

## 测试清单

- [ ] 点击"执行工具调用"按钮
- [ ] 按钮立即显示"执行中..."并禁用
- [ ] 工具执行完成后，按钮恢复"执行工具调用"
- [ ] 工具执行失败时，按钮也能恢复正常
- [ ] 多次快速点击不会导致状态错乱
- [ ] 切换聊天后，之前的执行状态不会影响新聊天

## 相关文件

- **API 接口**: `src/lib/api/message.ts`
- **状态管理**: `src/lib/states/message.svelte.ts`
- **UI 组件**: `src/lib/components/chat/views/AssistantMessageView.svelte`
- **后端实现**: `src-tauri/src/services/message.rs`
- **后端命令**: `src-tauri/src/commands/message.rs`

---

**更新日期**: 2025-10-03
