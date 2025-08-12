<script lang="ts">
import { onMount } from 'svelte';
import { 
  currentSession, 
  sessions, 
  messages, 
  chatLoading,
  chatActions 
} from '$lib/stores';
import { 
  providers, 
  selectedProvider, 
  selectedModel,
  providerActions 
} from '$lib/stores';

let messageInput = $state('');
let chatContainer: HTMLElement;

// 发送消息
async function sendMessage() {
  if (!messageInput.trim()) return;
  
  const message = messageInput.trim();
  messageInput = '';
  
  try {
    await chatActions.sendMessage(message);
    scrollToBottom();
  } catch (error) {
    console.error('Failed to send message:', error);
  }
}

// 滚动到底部
function scrollToBottom() {
  if (chatContainer) {
    chatContainer.scrollTop = chatContainer.scrollHeight;
  }
}

// 处理键盘事件
function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault();
    sendMessage();
  }
}

onMount(() => {
  // 初始化聊天数据
  // chatActions.loadSessions();
  // providerActions.loadProviders();
});
</script>

<div class="chat-layout">
  <!-- 会话侧边栏 -->
  <aside class="sessions-sidebar">
    <div class="sessions-header">
      <h3>Conversations</h3>
      <button class="new-chat-btn" onclick={() => chatActions.createSession()}>
        <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
        </svg>
        New Chat
      </button>
    </div>
    
    <div class="sessions-list">
      {#each $sessions as session (session.id)}
        <button 
          class="session-item"
          class:active={$currentSession?.id === session.id}
          onclick={() => chatActions.switchToSession(session.id)}
        >
          <div class="session-title">{session.name}</div>
          <div class="session-date">{new Date(session.updatedAt).toLocaleDateString()}</div>
        </button>
      {/each}
      
      {#if $sessions.length === 0}
        <div class="empty-state">
          <p>No conversations yet</p>
          <p class="empty-description">Start a new chat to begin</p>
        </div>
      {/if}
    </div>
  </aside>

  <!-- 主聊天区域 -->
  <div class="chat-main">
    <!-- 聊天头部 -->
    <header class="chat-header">
      <div class="chat-info">
        <h2>{$currentSession?.name || 'New Chat'}</h2>
        <div class="provider-info">
          {#if $selectedProvider}
            <span class="provider-name">{$selectedProvider.name}</span>
            <span class="provider-model">{$selectedModel?.name || $currentSession?.config.model || 'Default Model'}</span>
          {:else}
            <span class="no-provider">No provider selected</span>
          {/if}
        </div>
      </div>
      
      <div class="chat-actions">
        <button class="action-btn" title="Clear chat">
          <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
          </svg>
        </button>
      </div>
    </header>

    <!-- 消息列表 -->
    <div class="messages-container" bind:this={chatContainer}>
      <div class="messages">
        {#if $messages && $messages.length > 0}
          {#each $messages as message (message.id)}
            <div class="message" class:user={message.role === 'user'} class:assistant={message.role === 'assistant'}>
              <div class="message-avatar">
                {#if message.role === 'user'}
                  <svg width="16" height="16" fill="currentColor" viewBox="0 0 24 24">
                    <path d="M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z"/>
                  </svg>
                {:else}
                  <svg width="16" height="16" fill="currentColor" viewBox="0 0 24 24">
                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
                  </svg>
                {/if}
              </div>
              <div class="message-content">
                <div class="message-text">{message.content}</div>
                <div class="message-meta">
                  <span class="message-time">{new Date(message.createdAt).toLocaleTimeString()}</span>
                </div>
              </div>
            </div>
          {/each}
        {:else}
          <div class="welcome-state">
            <div class="welcome-icon">
              <svg width="48" height="48" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
              </svg>
            </div>
            <h3>Welcome to HandBox</h3>
            <p>Start a conversation by typing a message below</p>
          </div>
        {/if}
        
        {#if $chatLoading}
          <div class="message assistant">
            <div class="message-avatar">
              <svg width="16" height="16" fill="currentColor" viewBox="0 0 24 24">
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
              </svg>
            </div>
            <div class="message-content">
              <div class="typing-indicator">
                <span></span>
                <span></span>
                <span></span>
              </div>
            </div>
          </div>
        {/if}
      </div>
    </div>

    <!-- 输入区域 -->
    <div class="input-area">
      <div class="input-container">
        <textarea
          bind:value={messageInput}
          onkeydown={handleKeydown}
          placeholder="Type your message here..."
          rows="1"
          class="message-input"
          disabled={$chatLoading}
        ></textarea>
        <button 
          class="send-btn"
          onclick={sendMessage}
          disabled={!messageInput.trim() || $chatLoading}
        >
          <svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8" />
          </svg>
        </button>
      </div>
    </div>
  </div>
</div>

<style>
.chat-layout {
  display: flex;
  height: 100vh;
}

/* 会话侧边栏样式 */
.sessions-sidebar {
  width: 280px;
  border-right: 1px solid var(--border-color);
  background-color: var(--bg-secondary);
  display: flex;
  flex-direction: column;
}

.sessions-header {
  padding: 1rem;
  border-bottom: 1px solid var(--border-color);
}

.sessions-header h3 {
  margin: 0 0 1rem 0;
  font-size: 1.1rem;
  font-weight: 600;
}

.new-chat-btn {
  width: 100%;
  padding: 0.5rem;
  background: var(--bg-accent);
  color: var(--text-accent);
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-weight: 500;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  transition: opacity 0.2s;
}

.new-chat-btn:hover {
  opacity: 0.9;
}

.sessions-list {
  flex: 1;
  overflow-y: auto;
  padding: 0.5rem;
}

.session-item {
  width: 100%;
  padding: 0.75rem;
  background: none;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  text-align: left;
  margin-bottom: 0.25rem;
  transition: background-color 0.2s;
}

.session-item:hover {
  background-color: var(--bg-hover);
}

.session-item.active {
  background-color: var(--bg-accent);
  color: var(--text-accent);
}

.session-title {
  font-weight: 500;
  margin-bottom: 0.25rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.session-date {
  font-size: 0.75rem;
  opacity: 0.7;
}

.empty-state {
  text-align: center;
  padding: 2rem 1rem;
  color: var(--text-secondary);
}

.empty-description {
  font-size: 0.875rem;
  opacity: 0.7;
  margin-top: 0.5rem;
}

/* 主聊天区域样式 */
.chat-main {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.chat-header {
  padding: 1rem;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  justify-content: space-between;
  align-items: center;
  background-color: var(--bg-primary);
}

.chat-info h2 {
  margin: 0;
  font-size: 1.125rem;
  font-weight: 600;
}

.provider-info {
  margin-top: 0.25rem;
  font-size: 0.875rem;
  color: var(--text-secondary);
}

.provider-name {
  font-weight: 500;
}

.provider-model {
  margin-left: 0.5rem;
  opacity: 0.7;
}

.no-provider {
  color: #ef4444;
}

.chat-actions {
  display: flex;
  gap: 0.5rem;
}

.action-btn {
  padding: 0.5rem;
  background: none;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  cursor: pointer;
  color: var(--text-secondary);
  transition: all 0.2s;
}

.action-btn:hover {
  background-color: var(--bg-hover);
  color: var(--text-primary);
}

/* 消息区域样式 */
.messages-container {
  flex: 1;
  overflow-y: auto;
  padding: 1rem;
}

.messages {
  max-width: 800px;
  margin: 0 auto;
}

.message {
  display: flex;
  gap: 0.75rem;
  margin-bottom: 1.5rem;
}

.message-avatar {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  background-color: var(--bg-secondary);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.message.user .message-avatar {
  background-color: var(--bg-accent);
  color: var(--text-accent);
}

.message-content {
  flex: 1;
  min-width: 0;
}

.message-text {
  background-color: var(--bg-secondary);
  padding: 0.75rem 1rem;
  border-radius: 12px;
  line-height: 1.5;
}

.message.user .message-text {
  background-color: var(--bg-accent);
  color: var(--text-accent);
}

.message-meta {
  margin-top: 0.5rem;
  font-size: 0.75rem;
  color: var(--text-secondary);
}

.welcome-state {
  text-align: center;
  padding: 3rem 1rem;
  color: var(--text-secondary);
}

.welcome-icon {
  margin-bottom: 1rem;
  opacity: 0.5;
}

.welcome-state h3 {
  margin: 0 0 0.5rem 0;
  font-size: 1.25rem;
  font-weight: 600;
}

.welcome-state p {
  margin: 0;
  opacity: 0.7;
}

/* 打字指示器 */
.typing-indicator {
  display: flex;
  gap: 0.25rem;
  padding: 0.75rem 1rem;
  background-color: var(--bg-secondary);
  border-radius: 12px;
  width: fit-content;
}

.typing-indicator span {
  width: 6px;
  height: 6px;
  background-color: var(--text-secondary);
  border-radius: 50%;
  animation: typing 1.4s infinite;
}

.typing-indicator span:nth-child(2) {
  animation-delay: 0.2s;
}

.typing-indicator span:nth-child(3) {
  animation-delay: 0.4s;
}

@keyframes typing {
  0%, 60%, 100% {
    transform: translateY(0);
    opacity: 0.5;
  }
  30% {
    transform: translateY(-10px);
    opacity: 1;
  }
}

/* 输入区域样式 */
.input-area {
  padding: 1rem;
  border-top: 1px solid var(--border-color);
  background-color: var(--bg-primary);
}

.input-container {
  max-width: 800px;
  margin: 0 auto;
  position: relative;
}

.message-input {
  width: 100%;
  min-height: 44px;
  max-height: 200px;
  padding: 0.75rem 3rem 0.75rem 1rem;
  border: 1px solid var(--border-color);
  border-radius: 12px;
  background-color: var(--bg-primary);
  color: var(--text-primary);
  font-family: inherit;
  font-size: 0.875rem;
  line-height: 1.5;
  resize: none;
  outline: none;
  transition: border-color 0.2s;
}

.message-input:focus {
  border-color: var(--bg-accent);
}

.message-input:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.send-btn {
  position: absolute;
  right: 0.5rem;
  top: 50%;
  transform: translateY(-50%);
  width: 32px;
  height: 32px;
  background: var(--bg-accent);
  color: var(--text-accent);
  border: none;
  border-radius: 8px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: opacity 0.2s;
}

.send-btn:hover:not(:disabled) {
  opacity: 0.9;
}

.send-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .sessions-sidebar {
    position: fixed;
    left: 0;
    top: 0;
    height: 100vh;
    z-index: 1000;
    transform: translateX(-100%);
    transition: transform 0.3s ease;
  }
  
  .chat-layout {
    flex-direction: column;
  }
  
  .chat-main {
    margin-left: 0;
  }
}
</style>