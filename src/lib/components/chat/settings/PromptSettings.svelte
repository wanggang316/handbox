<script lang="ts">
  import Button from '../../ui/Button.svelte';
  import { MessageSquare, Save, RotateCcw, RefreshCw } from '@lucide/svelte';
  import RoundButton from '../../ui/RoundButton.svelte';
  import { TableGroup, TextareaRow } from '../../ui/table';

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

<div class="flex-1 p-0 space-y-6">

  <!-- 提示词编辑区 -->
  <TableGroup>
    <TextareaRow 
      label="系统提示词"
      bind:value={currentPrompt}
      placeholder="输入系统提示词..."
      rows={6}
      showCharCount={true}
      description=""
    />
  </TableGroup>

  <!-- 操作按钮 -->
  <div class="flex gap-3 justify-end">
    <RoundButton 
      customClass="w-18"
      label="保存" 
      onclick={handleSave} 
      disabled={!hasChanges}
    ></RoundButton>
    
  </div>
</div>


