<script lang="ts">
  import Select from '../ui/Select.svelte';
  import Toggle from '../ui/Toggle.svelte';
  import Button from '../ui/Button.svelte';
  import { Settings, Save, RotateCcw, Info } from '@lucide/svelte';

  interface Props {
    temperature?: string;
    topP?: string;
    streamResponse?: boolean;
    maxTokens?: string;
    contextLength?: string;
    onSave?: (settings: ModelSettingsData) => void;
  }

  interface ModelSettingsData {
    temperature: string;
    topP: string;
    streamResponse: boolean;
    maxTokens: string;
    contextLength: string;
  }

  let { 
    temperature = '0.7',
    topP = '0.9',
    streamResponse = true,
    maxTokens = '2048',
    contextLength = '4096',
    onSave
  }: Props = $props();

  let currentSettings = $state({
    temperature,
    topP,
    streamResponse,
    maxTokens,
    contextLength
  });

  const originalSettings = {
    temperature,
    topP,
    streamResponse,
    maxTokens,
    contextLength
  };

  let hasChanges = $derived(
    JSON.stringify(currentSettings) !== JSON.stringify(originalSettings)
  );

  const temperatureOptions = [
    { value: '0.1', label: '0.1 (保守)' },
    { value: '0.3', label: '0.3 (平衡)' },
    { value: '0.7', label: '0.7 (创意)' },
    { value: '1.0', label: '1.0 (随机)' }
  ];

  const topPOptions = [
    { value: '0.1', label: '0.1' },
    { value: '0.5', label: '0.5' },
    { value: '0.9', label: '0.9' },
    { value: '1.0', label: '1.0' }
  ];

  const contextLengthOptions = [
    { value: '2048', label: '2K tokens' },
    { value: '4096', label: '4K tokens' },
    { value: '8192', label: '8K tokens' },
    { value: '16384', label: '16K tokens' },
    { value: '32768', label: '32K tokens' }
  ];

  function handleSave() {
    onSave?.(currentSettings);
  }

  function handleReset() {
    currentSettings = { ...originalSettings };
  }

  function handleDefault() {
    currentSettings = {
      temperature: '0.7',
      topP: '0.9',
      streamResponse: true,
      maxTokens: '2048',
      contextLength: '4096'
    };
  }
</script>

<div class="flex-1 p-6 space-y-6">
  <!-- 标题 -->
  <div class="flex items-center gap-3">
    <Settings size={20} />
    <h3 class="text-lg font-medium text-gray-900">模型参数</h3>
  </div>

  <!-- 说明 -->
  <div class="text-sm text-gray-600 bg-yellow-50 p-4 rounded-lg">
    <div class="flex items-start gap-2">
      <Info size={16} class="mt-0.5 text-yellow-600" />
      <div>
        <p class="mb-2">这些参数控制模型的生成行为，调整前请确保了解其含义。</p>
        <p>不同模型对参数的敏感度可能不同，建议逐步调试。</p>
      </div>
    </div>
  </div>

  <!-- 参数设置 -->
  <div class="space-y-6">
    <!-- Temperature -->
    <div class="space-y-3">
      <div>
        <Select 
          label="创造性 (Temperature)"
          bind:value={currentSettings.temperature} 
          options={temperatureOptions}
        />
      </div>
      <div class="text-xs text-gray-500 ml-1">
        控制输出的随机性。较低值产生更确定的结果，较高值增加创造性。
      </div>
    </div>

    <!-- Top-P -->
    <div class="space-y-3">
      <div>
        <Select 
          label="核采样 (Top-p)"
          bind:value={currentSettings.topP} 
          options={topPOptions}
        />
      </div>
      <div class="text-xs text-gray-500 ml-1">
        控制词汇选择的多样性。较低值聚焦于最可能的词汇，较高值允许更多选择。
      </div>
    </div>

    <!-- 流式输出 -->
    <div class="space-y-3">
      <div class="flex items-center justify-between">
        <div>
          <label class="text-sm font-medium text-gray-700">流式输出</label>
          <div class="text-xs text-gray-500 mt-1">
            开启后，模型回答会逐字显示，提供更好的实时体验。
          </div>
        </div>
        <Toggle bind:checked={currentSettings.streamResponse} />
      </div>
    </div>

    <!-- 最大 Token 数 -->
    <div class="space-y-3">
      <label class="block text-sm font-medium text-gray-700">最大输出令牌</label>
      <input 
        type="number" 
        bind:value={currentSettings.maxTokens}
        min="256" 
        max="8192" 
        step="256"
        class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
      />
      <div class="text-xs text-gray-500">
        限制模型单次回答的最大长度。1 token 约等于 0.75 个英文单词。
      </div>
    </div>

    <!-- 上下文长度 -->
    <div class="space-y-3">
      <div>
        <Select 
          label="上下文长度"
          bind:value={currentSettings.contextLength} 
          options={contextLengthOptions}
        />
      </div>
      <div class="text-xs text-gray-500 ml-1">
        模型能够记住的对话历史长度。更长的上下文需要更多计算资源。
      </div>
    </div>
  </div>

  <!-- 操作按钮 -->
  <div class="flex gap-3 pt-4 border-t border-gray-200">
    <Button
      on:click={handleDefault}
      variant="secondary"
      size="sm"
    >
      恢复默认
    </Button>
    <Button
      on:click={handleReset}
      variant="secondary"
      disabled={!hasChanges}
    >
      <RotateCcw size={14} />
      重置
    </Button>
    <Button
      on:click={handleSave}
      variant="primary"
      disabled={!hasChanges}
    >
      <Save size={14} />
      保存
    </Button>
  </div>
</div>
