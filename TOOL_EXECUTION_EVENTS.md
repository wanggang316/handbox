# 工具执行事件说明

## 概述

在执行 MCP 工具调用时，后端会发送 `tool_execute` 事件来通知前端工具的执行状态。

## 事件类型

### `tool_execute` - 工具执行状态变化

当 MCP 工具执行状态变化时触发（开始执行/执行完成）。

**事件数据:**
```typescript
{
  messageId: string;       // 消息 ID
  toolCallIds: string[];   // 工具调用 ID 数组
  status: "executing" | "finished";  // 执行状态
}
```

**状态说明:**
- `executing` - 工具开始执行
- `finished` - 所有工具执行完成

## 前端使用示例

### TypeScript 示例

```typescript
import { listen } from '@tauri-apps/api/event';

// 监听工具执行状态
const unlisten = await listen('tool_execute', (event) => {
  const { messageId, toolCallIds, status } = event.payload;

  if (status === 'executing') {
    console.log(`Tools ${toolCallIds.join(', ')} started executing for message ${messageId}`);
    // 更新 UI 显示 "执行中..."
    updateToolStatus(messageId, 'executing');
  } else if (status === 'finished') {
    console.log(`Tools ${toolCallIds.join(', ')} finished executing for message ${messageId}`);
    // 更新 UI 恢复正常状态
    updateToolStatus(messageId, 'idle');
  }
});

// 清理监听器
onDestroy(() => {
  unlisten();
});
```

### Svelte 5 组件示例

```svelte
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';

  type ToolExecuteStatus = 'idle' | 'executing' | 'finished';

  interface ToolExecuteEvent {
    messageId: string;
    toolCallIds: string[];
    status: 'executing' | 'finished';
  }

  let toolStatuses = $state<Record<string, ToolExecuteStatus>>({});
  let unlisten: UnlistenFn | null = null;

  onMount(async () => {
    // 监听工具执行事件
    unlisten = await listen<ToolExecuteEvent>('tool_execute', (event) => {
      const { messageId, status } = event.payload;

      if (status === 'executing') {
        toolStatuses[messageId] = 'executing';
      } else if (status === 'finished') {
        toolStatuses[messageId] = 'idle';
      }
    });
  });

  onDestroy(() => {
    unlisten?.();
  });

  function getButtonText(messageId: string) {
    const status = toolStatuses[messageId] || 'idle';
    return status === 'executing' ? '执行中...' : '执行工具';
  }

  function isExecuting(messageId: string) {
    return toolStatuses[messageId] === 'executing';
  }
</script>

<button
  disabled={isExecuting(message.id)}
  onclick={() => executeToolCall(message.id, toolCallIds)}
>
  {getButtonText(message.id)}
</button>
```

## 完整工作流程

1. 用户点击 "执行工具" 按钮
2. 前端调用 `message_execute_tool_calls_stream(messageId, toolCallIds)`
3. 后端触发 `tool_execute_stream_start` 事件（流式开始）
4. **触发 `tool_execute` 事件，status = "executing"** → 前端显示 "执行中..."
5. 后端依次执行所有 MCP 工具调用
6. **触发 `tool_execute` 事件，status = "finished"** → 前端恢复按钮状态
7. 后端调用 LLM API 处理工具结果
8. 触发 `tool_execute_stream_chunk` 事件（LLM 响应流式数据）
9. 触发 `tool_execute_stream_end` 事件（完成）

## API 接口

### `message_execute_tool_calls_stream`

流式执行工具调用并获取 LLM 响应。

**参数:**
```typescript
{
  messageId: string;        // 包含工具调用的消息 ID
  toolCallIds: string[];    // 要执行的工具调用 ID 列表
}
```

**相关事件:**
- `tool_execute_stream_start` - 流式开始
- `tool_execute` - 工具执行状态变化（本文档重点）
- `tool_execute_stream_chunk` - LLM 响应数据块
- `tool_execute_stream_end` - 流式结束
- `tool_execute_stream_error` - 发生错误

## 状态管理最佳实践

### 1. 使用消息 ID 作为状态 key

```typescript
// ✅ 推荐：基于 messageId 管理状态
const toolStatuses = $state<Record<string, ToolExecuteStatus>>({});

// ❌ 不推荐：基于 toolCallId 管理（一个消息可能有多个工具调用）
const toolStatuses = $state<Record<string, ToolExecuteStatus>>({});
```

### 2. 处理状态转换

```typescript
listen<ToolExecuteEvent>('tool_execute', (event) => {
  const { messageId, status } = event.payload;

  switch (status) {
    case 'executing':
      // 禁用按钮，显示加载状态
      toolStatuses[messageId] = 'executing';
      break;
    case 'finished':
      // 恢复按钮，准备接收 LLM 响应
      toolStatuses[messageId] = 'idle';
      break;
  }
});
```

### 3. 错误处理

```typescript
// 监听错误事件
listen('tool_execute_stream_error', (event) => {
  const { streamId, error } = event.payload;

  // 根据 streamId 找到对应的 messageId，恢复状态
  toolStatuses[messageId] = 'idle';
  showError(error);
});
```

## 注意事项

- `tool_execute` 事件的 `status` 只有 `executing` 和 `finished` 两种状态
- `executing` 在开始执行所有工具前触发一次
- `finished` 在完成所有工具执行后触发一次
- 如果有多个工具调用，它们会依次执行，但只触发一次 `executing` 和一次 `finished`
- 工具执行失败时，`finished` 状态仍会触发，但结果中会包含错误信息
- 前端应根据 `messageId` 来管理按钮状态，而非 `toolCallId`
- `toolCallIds` 数组包含了本次执行的所有工具调用 ID，可用于日志和调试

## TypeScript 类型定义

```typescript
// 事件类型定义
interface ToolExecuteEvent {
  messageId: string;
  toolCallIds: string[];
  status: 'executing' | 'finished';
}

// 状态类型定义
type ToolExecuteStatus = 'idle' | 'executing' | 'finished';

// 监听器示例
const unlisten = await listen<ToolExecuteEvent>('tool_execute', (event) => {
  // event.payload 的类型自动推断为 ToolExecuteEvent
  const { messageId, toolCallIds, status } = event.payload;
});
```
