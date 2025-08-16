<script lang="ts">
  import LabeledSlider from '../ui/LabeledSlider.svelte';
  import SwitchRow from '../ui/SwitchRow.svelte';
  import NumberStepperRow from '../ui/NumberStepperRow.svelte';
  import Button from '../ui/Button.svelte';
  import TableGroup from '../ui/TableGroup.svelte';
  import TableRow from '../ui/TableRow.svelte';
  import { Save, RotateCcw, Info } from '@lucide/svelte';
    import RoundButton from '../ui/RoundButton.svelte';

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
    temperature: parseFloat(temperature),
    topP: parseFloat(topP),
    streamResponse,
    maxTokens: parseInt(maxTokens),
    contextLength: parseInt(contextLength)
  });

  const originalSettings = {
    temperature: parseFloat(temperature),
    topP: parseFloat(topP),
    streamResponse,
    maxTokens: parseInt(maxTokens),
    contextLength: parseInt(contextLength)
  };

  let hasChanges = $derived(
    JSON.stringify(currentSettings) !== JSON.stringify(originalSettings)
  );



  function handleSave() {
    // 转换回字符串格式，保持与原接口兼容
    const settingsToSave = {
      temperature: currentSettings.temperature.toString(),
      topP: currentSettings.topP.toString(),
      streamResponse: currentSettings.streamResponse,
      maxTokens: currentSettings.maxTokens.toString(),
      contextLength: currentSettings.contextLength.toString()
    };
    onSave?.(settingsToSave);
  }

  function handleReset() {
    currentSettings = { ...originalSettings };
  }

  function handleDefault() {
    currentSettings = {
      temperature: 0.7,
      topP: 0.9,
      streamResponse: true,
      maxTokens: 2048,
      contextLength: 4096
    };
  }
</script>

<div class="flex-1 p-0 space-y-6">

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
  <TableGroup>
    <!-- Temperature 和 Top-P 横向排列 -->
    <TableRow>
      <div class="flex flex-row gap-6 w-full">
        <div class="flex-1">
          <LabeledSlider 
            label="Temperature"
            bind:value={currentSettings.temperature}
            min={0.1}
            max={2.0}
            step={0.1}
            leftLabel="精确"
            rightLabel="创意"
            scaleMarks={[
              { value: 0, position: 0 },
              { value: 1, position: 47.37 },
              { value: 2, position: 100 }
            ]}
            description=""
          />
        </div>
        <div class="flex-1">
          <LabeledSlider 
            label="Top-p"
            bind:value={currentSettings.topP}
            min={0.1}
            max={1.0}
            step={0.1}
            leftLabel="聚焦"
            rightLabel="多样"
            scaleMarks={[
              { value: 0.1, position: 0 },
              { value: 0.5, position: 44.44 },
              { value: 1.0, position: 100 }
            ]}
            description=""
          />
        </div>
      </div>
    </TableRow>

    <!-- 流式输出 -->
    <TableRow>
      <SwitchRow 
        label="流式输出"
        bind:checked={currentSettings.streamResponse}
      />
    </TableRow>

    
  </TableGroup>

  <TableGroup>
    <!-- 最大 Token 数 -->
    <TableRow>
      <NumberStepperRow 
        label="最大输出长度"
        bind:value={currentSettings.maxTokens}
        min={256}
        max={8192}
        step={256}
      />
    </TableRow>

    <!-- 上下文长度 -->
    <TableRow>
      <NumberStepperRow 
        label="上下文长度"
        bind:value={currentSettings.contextLength}
        min={2048}
        max={32768}
        step={2048}
      />
    </TableRow>
  </TableGroup>

  <!-- 操作按钮 -->
  <div class="flex gap-3 pt-4 justify-end">

    <RoundButton 
      customClass="w-24"
      label="恢复默认" 
      bgColor="bg-gray-200"
      textColor="text-gray-600"
      hoverColor="hover:text-gray-800"
      on:click={handleDefault} 
    ></RoundButton>

    <RoundButton 
      customClass="w-18"
      label="重置" 
      bgColor="bg-gray-200"
      textColor="text-gray-600"
      hoverColor="hover:text-gray-800"
      on:click={handleReset} 
      disabled={!hasChanges}
    ></RoundButton>

    <RoundButton 
      customClass="w-18"
      label="保存" 
      on:click={handleSave} 
      disabled={!hasChanges}
    ></RoundButton>
  </div>
</div>
