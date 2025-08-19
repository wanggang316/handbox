<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { X, Plus, ChevronDown } from '@lucide/svelte';
  import type { ProviderConfig } from '$lib/types/provider';
  
  const dispatch = createEventDispatcher<{
    close: void;
    confirm: ProviderConfig;
  }>();
  
  let formData = {
    name: '',
    provider_type: 'custom-openai' as const,
    base_url: '',
    api_key: ''
  };
  
  let isLoading = false;
  let errors: Record<string, string> = {};
  let showDropdown = false;
  
  const providerTypes = [
    { value: 'custom-openai', label: 'OpenAI 兼容', icon: '🤖' },
    { value: 'custom-anthropic', label: 'Anthropic 兼容', icon: '🧠' }
  ];
  
  function validate() {
    errors = {};
    
    if (!formData.name.trim()) {
      errors.name = '请输入供应商名称';
    }
    
    if (!formData.base_url.trim()) {
      errors.base_url = '请输入Base URL';
    } else if (!isValidUrl(formData.base_url)) {
      errors.base_url = '请输入有效的URL';
    }
    
    if (!formData.api_key.trim()) {
      errors.api_key = '请输入API Key';
    }
    
    return Object.keys(errors).length === 0;
  }
  
  function isValidUrl(string: string) {
    try {
      new URL(string);
      return true;
    } catch (_) {
      return false;
    }
  }
  
  function handleClose() {
    dispatch('close');
  }
  
  async function handleConfirm() {
    if (!validate()) return;
    
    isLoading = true;
    try {
      const config: ProviderConfig = {
        name: formData.name,
        type: formData.provider_type,
        baseUrl: formData.base_url,
        apiKey: formData.api_key,
        enabled: false
      };
      
      dispatch('confirm', config);
    } catch (error) {
      console.error('Failed to create provider:', error);
    } finally {
      isLoading = false;
    }
  }

  function selectProviderType(type: string) {
    formData.provider_type = type as any;
    showDropdown = false;
  }

  $: selectedType = providerTypes.find(t => t.value === formData.provider_type);
</script>

<!-- 遮罩层 -->
<div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
  <!-- 弹窗容器 -->
  <div class="bg-white rounded-2xl max-w-2xl w-full max-h-[90vh] overflow-y-auto">
    <!-- 头部 -->
    <div class="flex items-center justify-between p-6 border-b border-slate-200">
      <div class="flex items-center gap-4">
        <!-- 供应商头像 -->
        <div class="w-16 h-16 rounded-2xl bg-gradient-to-br from-purple-500 to-blue-600 flex items-center justify-center">
          <Plus class="w-8 h-8 text-white" />
        </div>
        <div>
          <h2 class="text-xl font-semibold text-slate-900">添加供应商</h2>
          <p class="text-slate-600">配置自定义AI模型供应商</p>
        </div>
      </div>
      
      <button
        on:click={handleClose}
        class="p-2 text-slate-400 hover:text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
      >
        <X class="w-5 h-5" />
      </button>
    </div>
    
    <!-- 表单内容 -->
    <div class="p-6">
      <div class="bg-slate-50 rounded-2xl p-6">
        <div class="space-y-4">
          <!-- 供应商名称 -->
          <div>
            <label for="providerName" class="block text-sm font-medium text-slate-700 mb-2">
              供应商名称
            </label>
            <input
              id="providerName"
              type="text"
              bind:value={formData.name}
              placeholder="请输入供应商名称"
              class="w-full px-4 py-3 bg-white border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 transition-colors"
              class:border-red-300={errors.name}
              class:focus:ring-red-500={errors.name}
              class:focus:border-red-500={errors.name}
            />
            {#if errors.name}
              <p class="mt-1 text-sm text-red-600">{errors.name}</p>
            {/if}
          </div>
          
          <!-- 供应商类型 -->
          <div>
            <label for="providerType" class="block text-sm font-medium text-slate-700 mb-2">
              供应商类型
            </label>
            <div class="relative">
              <button
                type="button"
                on:click={() => showDropdown = !showDropdown}
                class="w-full flex items-center justify-between px-4 py-3 bg-white border border-slate-300 rounded-lg hover:bg-slate-50 transition-colors"
              >
                <div class="flex items-center gap-3">
                  <span class="text-lg">{selectedType?.icon}</span>
                  <span class="text-slate-900">{selectedType?.label}</span>
                </div>
                <ChevronDown class="w-5 h-5 text-slate-400 {showDropdown ? 'rotate-180' : ''}" />
              </button>
              
              {#if showDropdown}
                <div class="absolute top-full left-0 right-0 mt-1 bg-white border border-slate-300 rounded-lg shadow-lg z-10">
                  {#each providerTypes as type}
                    <button
                      type="button"
                      on:click={() => selectProviderType(type.value)}
                      class="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-slate-50 transition-colors first:rounded-t-lg last:rounded-b-lg"
                    >
                      <span class="text-lg">{type.icon}</span>
                      <span class="text-slate-900">{type.label}</span>
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
          
          <!-- Base URL -->
          <div>
            <label for="baseUrl" class="block text-sm font-medium text-slate-700 mb-2">
              Base URL
            </label>
            <input
              id="baseUrl"
              type="url"
              bind:value={formData.base_url}
              placeholder="https://api.openai.com/v1"
              class="w-full px-4 py-3 bg-white border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 transition-colors"
              class:border-red-300={errors.base_url}
              class:focus:ring-red-500={errors.base_url}
              class:focus:border-red-500={errors.base_url}
            />
            {#if errors.base_url}
              <p class="mt-1 text-sm text-red-600">{errors.base_url}</p>
            {/if}
          </div>
          
          <!-- API Key -->
          <div>
            <label for="apiKey" class="block text-sm font-medium text-slate-700 mb-2">
              API Key
            </label>
            <input
              id="apiKey"
              type="password"
              bind:value={formData.api_key}
              placeholder="输入API Key"
              class="w-full px-4 py-3 bg-white border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 transition-colors"
              class:border-red-300={errors.api_key}
              class:focus:ring-red-500={errors.api_key}
              class:focus:border-red-500={errors.api_key}
            />
            {#if errors.api_key}
              <p class="mt-1 text-sm text-red-600">{errors.api_key}</p>
            {/if}
          </div>
        </div>
      </div>
    </div>
    
    <!-- 底部按钮 -->
    <div class="flex items-center justify-end gap-3 p-6 border-t border-slate-200">
      <button
        on:click={handleClose}
        class="px-4 py-2 text-slate-600 bg-slate-100 rounded-lg hover:bg-slate-200 transition-colors"
      >
        取消
      </button>
      <button
        on:click={handleConfirm}
        disabled={isLoading}
        class="flex items-center gap-2 px-6 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
      >
        {#if isLoading}
          <div class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
          <span>创建中...</span>
        {:else}
          <span>确认创建</span>
        {/if}
      </button>
    </div>
  </div>
</div>

<!-- 点击外部关闭下拉菜单 -->
{#if showDropdown}
  <div
    class="fixed inset-0 z-0"
    on:click={() => showDropdown = false}
    role="button"
    tabindex="-1"
  ></div>
{/if}