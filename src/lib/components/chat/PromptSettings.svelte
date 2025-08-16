<script lang="ts">
  import Button from '../ui/Button.svelte';
  import { MessageSquare, Save, RotateCcw } from '@lucide/svelte';
    import RoundButton from '../ui/RoundButton.svelte';

  interface Props {
    systemPrompt?: string;
    onSave?: (prompt: string) => void;
  }

  let { systemPrompt = '', onSave }: Props = $props();
  
  let currentPrompt = $state(systemPrompt);
  let hasChanges = $derived(currentPrompt !== systemPrompt);

  function handleSave() {
    onSave?.(currentPrompt);
  }

  function handleReset() {
    currentPrompt = systemPrompt;
  }

</script>

<div class="flex-1 p-6 space-y-6">

  <!-- 说明 -->
  <div class="text-sm text-gray-600 bg-blue-50 p-4 rounded-lg">
    <p class="mb-2">系统提示词用于定义AI助手的行为、角色和回答风格。</p>
    <p>提示：编写清晰、具体的指令可以显著提高AI的回答质量。</p>
  </div>

  <!-- 提示词编辑区 -->
  <div class="space-y-3">
    <label for="system-prompt" class="block text-sm font-medium text-gray-700">
      系统提示词
    </label>
    <textarea
      id="system-prompt"
      bind:value={currentPrompt}
      placeholder="输入系统提示词..."
      rows="12"
      class="w-full px-3 py-2 border border-gray-300 rounded-md resize-none focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent font-mono text-sm
             scrollbar-thin scrollbar-thumb-gray-300 scrollbar-track-gray-100 hover:scrollbar-thumb-gray-400"
    ></textarea>
    <div class="text-xs text-gray-500">
      Tokens: {currentPrompt.length}
    </div>
  </div>

  <!-- 操作按钮 -->
  <div class="flex gap-3 justify-end">
    <RoundButton 
      customClass="w-18"
      label="保存" 
      on:click={handleSave} 
      disabled={!hasChanges}
    ></RoundButton>
    
  </div>
</div>


