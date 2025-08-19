<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { X, Search, Check } from '@lucide/svelte';
  import type { Model } from '$lib/types/provider';
  
  export let providerId: string;
  
  const dispatch = createEventDispatcher<{
    close: void;
    confirm: { selectedModels: string[] };
  }>();
  
  let searchQuery = '';
  let selectedModels = new Set<string>();
  let isLoading = false;
  
  // 模拟模型数据 - 在实际实现中应该从props传入或API获取
  const mockModels: { provider: string; models: Model[] }[] = [
    {
      provider: 'OpenAI',
      models: [
        {
          id: 'gpt-5-chat',
          name: 'OpenAI: GPT-5 Chat',
          provider: 'openai',
          enabled: true,
          supportedFeatures: ['text', 'vision', 'function-calling']
        },
        {
          id: 'gpt-oss-20b',
          name: 'OpenAI: gpt-oss-20b (free)',
          provider: 'openai',
          enabled: true,
          supportedFeatures: ['text']
        },
        {
          id: 'gpt-5-mini',
          name: 'OpenAI: GPT-5 Mini',
          provider: 'openai',
          enabled: false,
          supportedFeatures: ['text', 'function-calling']
        }
      ]
    },
    {
      provider: 'Google',
      models: [
        {
          id: 'gemini-2.5-flash-lite',
          name: 'Google: Gemini 2.5 Flash Lite',
          provider: 'google',
          enabled: false,
          supportedFeatures: ['text', 'vision']
        },
        {
          id: 'gemini-2.5-flash',
          name: 'Google: Gemini 2.5 Flash',
          provider: 'google',
          enabled: false,
          supportedFeatures: ['text', 'vision', 'function-calling']
        },
        {
          id: 'gemini-2.5-pro',
          name: 'Google: Gemini 2.5 Pro',
          provider: 'google',
          enabled: true,
          supportedFeatures: ['text', 'vision', 'function-calling', 'reasoning']
        }
      ]
    }
  ];
  
  // 过滤模型
  $: filteredModels = mockModels.map(group => ({
    ...group,
    models: group.models.filter(model => 
      model.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      model.id.toLowerCase().includes(searchQuery.toLowerCase())
    )
  })).filter(group => group.models.length > 0);
  
  // 初始化已选中的模型
  $: {
    // 将已启用的模型添加到选中列表
    for (const group of mockModels) {
      for (const model of group.models) {
        if (model.enabled) {
          selectedModels.add(model.id);
        }
      }
    }
  }
  
  function toggleModel(modelId: string) {
    if (selectedModels.has(modelId)) {
      selectedModels.delete(modelId);
    } else {
      selectedModels.add(modelId);
    }
    selectedModels = selectedModels; // 触发响应式更新
  }
  
  function handleClose() {
    dispatch('close');
  }
  
  async function handleConfirm() {
    isLoading = true;
    try {
      dispatch('confirm', {
        selectedModels: Array.from(selectedModels)
      });
    } finally {
      isLoading = false;
    }
  }
</script>

<!-- 遮罩层 -->
<div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4">
  <!-- 弹窗容器 -->
  <div class="bg-white rounded-2xl w-full max-w-4xl max-h-[90vh] overflow-hidden flex flex-col">
    <!-- 头部 -->
    <div class="flex items-center justify-between p-6 border-b border-slate-200">
      <div class="flex items-center gap-4">
        <h2 class="text-2xl font-normal text-slate-900">选择模型</h2>
      </div>
      
      <button
        on:click={handleClose}
        class="p-2 text-slate-400 hover:text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
      >
        <X class="w-5 h-5" />
      </button>
    </div>

    <!-- 搜索区域 -->
    <div class="p-6 border-b border-slate-200">
      <div class="relative max-w-sm">
        <Search class="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-slate-400" />
        <input
          type="text"
          placeholder="搜索模型..."
          bind:value={searchQuery}
          class="w-full pl-10 pr-4 py-2 bg-slate-100 border-0 rounded-lg focus:ring-2 focus:ring-blue-500 focus:bg-white transition-colors"
        />
      </div>
    </div>
    
    <!-- 模型列表 -->
    <div class="flex-1 overflow-y-auto p-6">
      <div class="space-y-6">
        {#each filteredModels as group}
          <div class="bg-slate-50 rounded-3xl overflow-hidden">
            <!-- 供应商头部 -->
            <div class="px-6 py-4">
              <h3 class="text-lg font-medium text-slate-700">{group.provider}</h3>
            </div>
            
            <!-- 模型列表 -->
            <div class="space-y-0">
              {#each group.models as model}
                <label class="flex items-center gap-4 px-6 py-4 hover:bg-slate-100 cursor-pointer transition-colors border-t border-slate-200 first:border-t-0">
                  <div class="relative">
                    <input 
                      type="checkbox" 
                      checked={selectedModels.has(model.id)}
                      on:change={() => toggleModel(model.id)}
                      class="sr-only peer"
                    />
                    <div class="w-5 h-5 border-2 border-slate-300 rounded flex items-center justify-center transition-colors peer-checked:bg-blue-600 peer-checked:border-blue-600">
                      <Check class="w-3 h-3 text-white opacity-0 peer-checked:opacity-100 transition-opacity" />
                    </div>
                  </div>
                  <div class="flex-1">
                    <div class="text-lg text-slate-900">{model.name}</div>
                    <div class="text-sm text-slate-500 mt-1">
                      功能: {model.supportedFeatures.join(', ')}
                    </div>
                  </div>
                  <div class="text-sm text-slate-500">
                    {model.enabled ? '已启用' : '未启用'}
                  </div>
                </label>
              {/each}
            </div>
          </div>
        {/each}
        
        {#if filteredModels.length === 0}
          <div class="text-center py-12">
            <p class="text-slate-500">没有找到匹配的模型</p>
          </div>
        {/if}
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
        disabled={isLoading || selectedModels.size === 0}
        class="flex items-center gap-2 px-6 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
      >
        {#if isLoading}
          <div class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
          <span>确认中...</span>
        {:else}
          <span>确认 ({selectedModels.size})</span>
        {/if}
      </button>
    </div>
  </div>
</div>