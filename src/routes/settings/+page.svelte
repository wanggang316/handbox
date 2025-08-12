<script lang="ts">
import { onMount } from 'svelte';
import { 
  providers,
  selectedProvider,
  providerActions,
  appSettings,
  settingsActions
} from '$lib/stores';
import type { ProviderConfig, Provider } from '$lib/types';

let activeTab = $state('providers');
let showAddProvider = $state(false);
let newProvider: Partial<ProviderConfig> = $state({
  name: '',
  type: 'openai',
  apiKey: '',
  baseUrl: ''
});

// 切换标签页
function switchTab(tab: string) {
  activeTab = tab;
}

// 添加供应商
async function addProvider() {
  if (!newProvider.type || !newProvider.apiKey) return;
  try {
    await providerActions.createProvider(newProvider as ProviderConfig);
    showAddProvider = false;
    resetNewProvider();
  } catch (error) {
    console.error('Failed to add provider:', error);
  }
}

// 删除供应商
async function removeProvider(providerId: string) {
  if (confirm('Are you sure you want to remove this provider?')) {
    try {
      await providerActions.deleteProvider(providerId);
    } catch (error) {
      console.error('Failed to remove provider:', error);
    }
  }
}

// 重置新供应商表单
function resetNewProvider() {
  newProvider = {
    name: '',
    type: 'openai',
    apiKey: '',
    baseUrl: ''
  };
}

// 获取供应商类型的显示名称
function getProviderTypeName(type: string): string {
  switch (type) {
    case 'openai': return 'OpenAI';
    case 'anthropic': return 'Anthropic';
    case 'google': return 'Google AI';
    case 'deepseek': return 'DeepSeek';
    case 'openrouter': return 'OpenRouter';
    case 'custom-openai': return 'Custom OpenAI';
    case 'custom-anthropic': return 'Custom Anthropic';
    default: return type;
  }
}

function getProviderModelText(provider: Provider): string {
  const firstEnabled = provider.models?.find(m => m.enabled);
  return firstEnabled?.name || provider.models?.[0]?.name || 'Default Model';
}

onMount(() => {
  providerActions.loadProviders();
  settingsActions.loadSettings();
});
</script>

<div class="settings-container">
  <div class="settings-header">
    <h1>Settings</h1>
    <p>Configure your HandBox experience</p>
  </div>

  <!-- 标签页导航 -->
  <div class="tabs">
    <button 
      class="tab"
      class:active={activeTab === 'providers'}
      onclick={() => switchTab('providers')}
    >
      Providers
    </button>
    <button 
      class="tab"
      class:active={activeTab === 'general'}
      onclick={() => switchTab('general')}
    >
      General
    </button>
    <button 
      class="tab"
      class:active={activeTab === 'appearance'}
      onclick={() => switchTab('appearance')}
    >
      Appearance
    </button>
  </div>

  <!-- 标签页内容 -->
  <div class="tab-content">
    {#if activeTab === 'providers'}
      <div class="section">
        <div class="section-header">
          <h2>AI Providers</h2>
          <button 
            class="btn-primary"
            onclick={() => showAddProvider = true}
          >
            Add Provider
          </button>
        </div>
        
        <div class="providers-list">
          {#each $providers as provider (provider.id)}
            <div class="provider-card">
              <div class="provider-info">
                <h3>{provider.name}</h3>
                <p class="provider-type">{getProviderTypeName(provider.type)}</p>
                <p class="provider-model">{getProviderModelText(provider)}</p>
              </div>
              <div class="provider-actions">
                <button 
                  class="btn-secondary"
                  onclick={() => providerActions.selectProvider(provider)}
                  disabled={$selectedProvider?.id === provider.id}
                >
                  {$selectedProvider?.id === provider.id ? 'Active' : 'Use'}
                </button>
                <button 
                  class="btn-danger"
                  onclick={() => removeProvider(provider.id)}
                >
                  Remove
                </button>
              </div>
            </div>
          {/each}
          
          {#if $providers.length === 0}
            <div class="empty-state">
              <p>No providers configured</p>
              <p class="empty-description">Add an AI provider to start chatting</p>
            </div>
          {/if}
        </div>
      </div>
    {/if}

    {#if activeTab === 'general'}
      <div class="section">
        <h2>General Settings</h2>
        {#if $appSettings}
          <div class="setting-group">
            <label class="setting-label">
              <input 
                type="checkbox" 
                bind:checked={$appSettings.general.autoScroll}
                onchange={() => settingsActions.updateSettings({ section: 'general', data: { autoScroll: $appSettings.general.autoScroll } })}
              />
              Auto-scroll to latest message
            </label>
          </div>

          <div class="setting-group">
            <label>Theme</label>
            <select 
              bind:value={$appSettings.general.theme}
              onchange={() => settingsActions.updateSettings({ section: 'general', data: { theme: $appSettings.general.theme } })}
              class="select-input"
            >
              <option value="system">System</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </div>

          <div class="setting-group">
            <label>Theme color</label>
            <select 
              bind:value={$appSettings.general.themeColor}
              onchange={() => settingsActions.updateSettings({ section: 'general', data: { themeColor: $appSettings.general.themeColor } })}
              class="select-input"
            >
              <option value="system">System</option>
              <option value="blue">Blue</option>
              <option value="green">Green</option>
              <option value="red">Red</option>
              <option value="yellow">Yellow</option>
              <option value="purple">Purple</option>
              <option value="orange">Orange</option>
              <option value="pink">Pink</option>
              <option value="brown">Brown</option>
            </select>
          </div>
        {:else}
          <div class="empty-state">
            <p>Loading settings...</p>
          </div>
        {/if}
      </div>
    {/if}

    {#if activeTab === 'appearance'}
      <div class="section">
        <h2>Appearance</h2>
        {#if $appSettings}
          <div class="setting-group">
            <label>Language</label>
            <select 
              bind:value={$appSettings.general.language}
              onchange={() => settingsActions.updateSettings({ section: 'general', data: { language: $appSettings.general.language } })}
              class="select-input"
            >
              <option value="zh-CN">简体中文</option>
              <option value="en-US">English</option>
            </select>
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<!-- 添加供应商模态框 -->
{#if showAddProvider}
  <div class="modal-overlay" onclick={(e) => {
    if (e.target === e.currentTarget) {
      showAddProvider = false;
      resetNewProvider();
    }
  }}>
    <div class="modal">
      <div class="modal-header">
        <h3>Add AI Provider</h3>
        <button 
          class="close-btn"
          onclick={() => {
            showAddProvider = false;
            resetNewProvider();
          }}
        >
          <svg width="20" height="20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
      
      <form class="modal-content" onsubmit={(e) => {
        e.preventDefault();
        addProvider();
      }}>
        <div class="form-group">
          <label for="provider-name">Name</label>
          <input 
            id="provider-name"
            type="text" 
            bind:value={newProvider.name}
            placeholder="e.g., OpenAI GPT-4"
            required
          />
        </div>
        
        <div class="form-group">
          <label for="provider-type">Type</label>
          <select id="provider-type" bind:value={newProvider.type} required>
            <option value="openai">OpenAI</option>
            <option value="anthropic">Anthropic</option>
            <option value="google">Google AI</option>
            <option value="deepseek">DeepSeek</option>
            <option value="openrouter">OpenRouter</option>
            <option value="custom-openai">Custom OpenAI</option>
            <option value="custom-anthropic">Custom Anthropic</option>
          </select>
        </div>
        
        <div class="form-group">
          <label for="provider-api-key">API Key</label>
          <input 
            id="provider-api-key"
            type="password" 
            bind:value={newProvider.apiKey}
            placeholder="Your API key"
            required
          />
        </div>
        
        <div class="form-group">
          <label for="provider-base-url">Base URL (Optional)</label>
          <input 
            id="provider-base-url"
            type="url" 
            bind:value={newProvider.baseUrl}
            placeholder="https://api.openai.com/v1"
          />
        </div>
        
        <div class="form-group">
          <label for="provider-model">Model (Optional)</label>
          <input 
            id="provider-model"
            type="text" 
            placeholder="gpt-4, claude-3-5-sonnet-20241022, etc."
          />
        </div>
        
        <div class="modal-actions">
          <button 
            type="button" 
            class="btn-secondary"
            onclick={() => {
              showAddProvider = false;
              resetNewProvider();
            }}
          >
            Cancel
          </button>
          <button type="submit" class="btn-primary">
            Add Provider
          </button>
        </div>
      </form>
    </div>
  </div>
{/if}

<style>
.settings-container {
  max-width: 800px;
  margin: 0 auto;
  padding: 2rem;
}

.settings-header {
  margin-bottom: 2rem;
}

.settings-header h1 {
  margin: 0 0 0.5rem 0;
  font-size: 2rem;
  font-weight: 700;
}

.settings-header p {
  margin: 0;
  color: var(--text-secondary);
}

/* 标签页样式 */
.tabs {
  display: flex;
  border-bottom: 1px solid var(--border-color);
  margin-bottom: 2rem;
}

.tab {
  padding: 0.75rem 1.5rem;
  background: none;
  border: none;
  border-bottom: 2px solid transparent;
  cursor: pointer;
  font-weight: 500;
  color: var(--text-secondary);
  transition: all 0.2s;
}

.tab:hover {
  color: var(--text-primary);
}

.tab.active {
  color: var(--bg-accent);
  border-bottom-color: var(--bg-accent);
}

/* 内容区域样式 */
.section {
  margin-bottom: 2rem;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1.5rem;
}

.section h2 {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 600;
}

/* 按钮样式 */
.btn-primary {
  background: var(--bg-accent);
  color: var(--text-accent);
  border: none;
  border-radius: 6px;
  padding: 0.5rem 1rem;
  font-weight: 500;
  cursor: pointer;
  transition: opacity 0.2s;
}

.btn-primary:hover {
  opacity: 0.9;
}

.btn-secondary {
  background: var(--bg-secondary);
  color: var(--text-primary);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 0.5rem 1rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-secondary:hover {
  background: var(--bg-hover);
}

.btn-secondary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-danger {
  background: #ef4444;
  color: white;
  border: none;
  border-radius: 6px;
  padding: 0.5rem 1rem;
  font-weight: 500;
  cursor: pointer;
  transition: opacity 0.2s;
}

.btn-danger:hover {
  opacity: 0.9;
}

/* 供应商列表样式 */
.providers-list {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.provider-card {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1.5rem;
  background: var(--bg-secondary);
  border-radius: 8px;
  border: 1px solid var(--border-color);
}

.provider-info h3 {
  margin: 0 0 0.5rem 0;
  font-size: 1.125rem;
  font-weight: 600;
}

.provider-type {
  margin: 0 0 0.25rem 0;
  font-size: 0.875rem;
  color: var(--text-secondary);
  font-weight: 500;
}

.provider-model {
  margin: 0;
  font-size: 0.75rem;
  color: var(--text-secondary);
  opacity: 0.7;
}

.provider-actions {
  display: flex;
  gap: 0.5rem;
}

.empty-state {
  text-align: center;
  padding: 3rem 1rem;
  color: var(--text-secondary);
}

.empty-description {
  font-size: 0.875rem;
  opacity: 0.7;
  margin-top: 0.5rem;
}

/* 设置组样式 */
.setting-group {
  margin-bottom: 1.5rem;
}

.setting-group label {
  display: block;
  margin-bottom: 0.5rem;
  font-weight: 500;
}

.setting-label {
  display: flex;
  align-items: center;
  cursor: pointer;
}

.setting-label input[type="checkbox"] {
  margin-right: 0.5rem;
}

.number-input,
.select-input {
  width: 200px;
  padding: 0.5rem;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: var(--bg-primary);
  color: var(--text-primary);
}

/* 模态框样式 */
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal {
  background: var(--bg-primary);
  border-radius: 8px;
  box-shadow: 0 10px 25px rgba(0, 0, 0, 0.15);
  max-width: 500px;
  width: 90%;
  max-height: 90vh;
  overflow-y: auto;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 1.5rem;
  border-bottom: 1px solid var(--border-color);
}

.modal-header h3 {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 600;
}

.close-btn {
  background: none;
  border: none;
  cursor: pointer;
  color: var(--text-secondary);
  padding: 0.25rem;
  border-radius: 4px;
  transition: background-color 0.2s;
}

.close-btn:hover {
  background: var(--bg-hover);
}

.modal-content {
  padding: 1.5rem;
}

.form-group {
  margin-bottom: 1.5rem;
}

.form-group label {
  display: block;
  margin-bottom: 0.5rem;
  font-weight: 500;
}

.form-group input,
.form-group select {
  width: 100%;
  padding: 0.75rem;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: var(--bg-primary);
  color: var(--text-primary);
  font-size: 0.875rem;
}

.form-group input:focus,
.form-group select:focus {
  outline: none;
  border-color: var(--bg-accent);
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 2rem;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .settings-container {
    padding: 1rem;
  }
  
  .provider-card {
    flex-direction: column;
    align-items: flex-start;
    gap: 1rem;
  }
  
  .provider-actions {
    width: 100%;
    justify-content: flex-end;
  }
}
</style>