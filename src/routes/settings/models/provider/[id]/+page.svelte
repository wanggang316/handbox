<script lang="ts">
  import { page } from '$app/stores';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { 
    ArrowLeft, 
    Eye, 
    EyeOff, 
    TestTube, 
    RotateCw, 
    Settings, 
    Trash2,
    ChevronDown
  } from '@lucide/svelte';
  import ModelSelectModal from '$lib/components/settings/ModelSelectModal.svelte';
  import type { Provider } from '$lib/types/provider';
  
  let providerId = '';
  let provider: Provider | null = null;
  let isLoading = false;
  let showModelsModal = false;
  let showDeleteConfirm = false;
  let showApiKey = false;
  
  // 配置表单
  let formData = {
    baseUrl: 'https://api.openai.com/v1',
    apiKey: '',
    enabled: true
  };

  // 模拟数据
  const mockProvider = {
    name: 'OpenAI',
    icon: '🤖',
    type: 'openai'
  };

  const mockModels = [
    { name: 'GPT-4o', id: 'gpt-4o', enabled: true },
    { name: 'GPT-4o Mini', id: 'gpt-4o-mini', enabled: true },
    { name: 'GPT-3.5 Turbo', id: 'gpt-3.5-turbo', enabled: false },
  ];
  
  onMount(() => {
    providerId = $page.params.id || '';
    loadProvider();
  });
  
  async function loadProvider() {
    // TODO: 从API加载供应商数据
    console.log('Loading provider:', providerId);
  }
  
  async function handleProbe() {
    if (!mockProvider) return;
    isLoading = true;
    try {
      // TODO: 实现探活检测
      await new Promise(resolve => setTimeout(resolve, 1500));
      console.log('Probing provider:', mockProvider.name);
    } catch (error) {
      console.error('Probe failed:', error);
    } finally {
      isLoading = false;
    }
  }
  
  async function handleSave() {
    if (!mockProvider) return;
    isLoading = true;
    try {
      // TODO: 保存配置
      await new Promise(resolve => setTimeout(resolve, 1000));
      console.log('Saving provider config:', formData);
    } catch (error) {
      console.error('Save failed:', error);
    } finally {
      isLoading = false;
    }
  }
  
  async function handleFetchModels() {
    if (!mockProvider) return;
    showModelsModal = true;
  }
  
  function handleModelsConfirm(event: CustomEvent<{ selectedModels: string[] }>) {
    const { selectedModels } = event.detail;
    console.log('Selected models:', selectedModels);
    showModelsModal = false;
  }
  
  function handleCloseModels() {
    showModelsModal = false;
  }
  
  async function handleDelete() {
    if (!mockProvider) return;
    showDeleteConfirm = true;
  }
  
  async function confirmDelete() {
    if (!mockProvider) return;
    isLoading = true;
    try {
      await new Promise(resolve => setTimeout(resolve, 1000));
      console.log('Deleting provider:', mockProvider.name);
      goto('/settings/models');
    } catch (error) {
      console.error('Delete failed:', error);
    } finally {
      isLoading = false;
      showDeleteConfirm = false;
    }
  }
  
  function handleBack() {
    goto('/settings/models');
  }
</script>

<div class="min-h-screen bg-white">
  <div class="max-w-4xl mx-auto p-8">
    <!-- 页面头部 -->
    <div class="mb-8">
      <button
        class="flex items-center gap-2 text-slate-600 hover:text-slate-900 transition-colors mb-4"
        on:click={handleBack}
      >
        <ArrowLeft class="w-4 h-4" />
        <span>返回</span>
      </button>
      <h1 class="text-2xl font-normal text-slate-900">模型供应商配置</h1>
    </div>
    
    <!-- 供应商信息卡片 -->
    <div class="bg-slate-50 rounded-2xl p-6 mb-6">
      <div class="flex items-center gap-4">
        <!-- 供应商图标 -->
        <div class="w-12 h-12 rounded-xl bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center text-white text-2xl">
          {mockProvider.icon}
        </div>
        
        <!-- 供应商信息 -->
        <div class="flex-1">
          <h2 class="text-xl font-medium text-slate-900">{mockProvider.name}</h2>
          <p class="text-slate-600">供应商类型: {mockProvider.type}</p>
        </div>
        
        <!-- 启用开关 -->
        <label class="flex items-center gap-3 cursor-pointer">
          <span class="text-sm font-medium text-slate-700">启用</span>
          <div class="relative">
            <input
              type="checkbox"
              bind:checked={formData.enabled}
              class="sr-only peer"
            />
            <div class="w-11 h-6 bg-slate-300 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-slate-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
          </div>
        </label>
      </div>
    </div>
    
    <!-- 配置表单 -->
    <div class="bg-slate-50 rounded-2xl p-6 mb-6">
      <h3 class="text-lg font-medium text-slate-900 mb-4">基础配置</h3>
      
      <div class="space-y-4">
        <!-- Base URL -->
        <div>
          <label for="baseUrl" class="block text-sm font-medium text-slate-700 mb-2">
            Base URL
          </label>
          <input
            id="baseUrl"
            type="url"
            bind:value={formData.baseUrl}
            placeholder="https://api.openai.com/v1"
            class="w-full px-4 py-3 bg-white border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 transition-colors"
          />
        </div>
        
        <!-- API Key -->
        <div>
          <label for="apiKey" class="block text-sm font-medium text-slate-700 mb-2">
            API Key
          </label>
          <div class="flex gap-2">
            <div class="relative flex-1">
              <input
                id="apiKey"
                type={showApiKey ? 'text' : 'password'}
                bind:value={formData.apiKey}
                placeholder="输入API Key"
                class="w-full px-4 py-3 pr-12 bg-white border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 transition-colors"
              />
              <button
                type="button"
                on:click={() => showApiKey = !showApiKey}
                class="absolute right-3 top-1/2 transform -translate-y-1/2 text-slate-400 hover:text-slate-600"
              >
                {#if showApiKey}
                  <EyeOff class="w-5 h-5" />
                {:else}
                  <Eye class="w-5 h-5" />
                {/if}
              </button>
            </div>
            
            <!-- 检测按钮 -->
            <button
              on:click={handleProbe}
              disabled={isLoading || !formData.apiKey}
              class="flex items-center gap-2 px-4 py-3 bg-white border border-slate-300 rounded-lg hover:bg-slate-50 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {#if isLoading}
                <RotateCw class="w-4 h-4 animate-spin" />
                <span>检测中</span>
              {:else}
                <TestTube class="w-4 h-4" />
                <span>检测</span>
              {/if}
            </button>
          </div>
        </div>
      </div>
    </div>
    
    <!-- 模型管理 -->
    <div class="bg-slate-50 rounded-2xl p-6 mb-6">
      <div class="flex items-center justify-between mb-4">
        <h3 class="text-lg font-medium text-slate-900">模型管理</h3>
        <button
          on:click={handleFetchModels}
          class="flex items-center gap-2 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
        >
          <RotateCw class="w-4 h-4" />
          获取模型列表
        </button>
      </div>
      
      <!-- 模型列表 -->
      <div class="space-y-2">
        {#each mockModels as model}
          <div class="flex items-center justify-between p-4 bg-white rounded-lg border border-slate-200">
            <div class="flex items-center gap-3">
              <input
                type="checkbox"
                bind:checked={model.enabled}
                class="w-4 h-4 text-blue-600 bg-white border-slate-300 rounded focus:ring-blue-500"
              />
              <span class="text-slate-900 font-medium">{model.name}</span>
              <span class="text-sm text-slate-500">({model.id})</span>
            </div>
            
            <div class="flex items-center gap-2">
              <button class="p-1 text-slate-400 hover:text-slate-600 transition-colors">
                <Settings class="w-4 h-4" />
              </button>
              <button class="p-1 text-slate-400 hover:text-red-600 transition-colors">
                <Trash2 class="w-4 h-4" />
              </button>
            </div>
          </div>
        {/each}
      </div>
    </div>
    
    <!-- 操作按钮 -->
    <div class="flex items-center justify-between">
      <button
        on:click={handleDelete}
        class="flex items-center gap-2 px-4 py-2 text-red-600 bg-red-50 border border-red-200 rounded-lg hover:bg-red-100 transition-colors"
      >
        <Trash2 class="w-4 h-4" />
        删除供应商
      </button>
      
      <button
        on:click={handleSave}
        disabled={isLoading}
        class="flex items-center gap-2 px-6 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
      >
        {#if isLoading}
          <RotateCw class="w-4 h-4 animate-spin" />
          <span>保存中...</span>
        {:else}
          <span>保存配置</span>
        {/if}
      </button>
    </div>
  </div>
</div>

<!-- 模型选择弹窗 -->
{#if showModelsModal}
  <ModelSelectModal 
    {providerId}
    on:close={handleCloseModels}
    on:confirm={handleModelsConfirm}
  />
{/if}

<!-- 删除确认弹窗 -->
{#if showDeleteConfirm}
  <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
    <div class="bg-white rounded-2xl p-6 max-w-md mx-4">
      <h3 class="text-lg font-semibold text-slate-900 mb-2">确认删除</h3>
      <p class="text-slate-600 mb-6">确定要删除该供应商吗？此操作不可撤销，所有相关配置和数据都将被清除。</p>
      
      <div class="flex items-center justify-end gap-3">
        <button
          on:click={() => showDeleteConfirm = false}
          class="px-4 py-2 text-slate-600 bg-slate-100 rounded-lg hover:bg-slate-200 transition-colors"
        >
          取消
        </button>
        <button
          on:click={confirmDelete}
          disabled={isLoading}
          class="flex items-center gap-2 px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {#if isLoading}
            <RotateCw class="w-4 h-4 animate-spin" />
            <span>删除中...</span>
          {:else}
            <Trash2 class="w-4 h-4" />
            <span>确认删除</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}