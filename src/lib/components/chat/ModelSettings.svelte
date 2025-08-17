<script lang="ts">
  import LabeledSliderRow from '../ui/table/LabeledSliderRow.svelte';
  import SwitchRow from '../ui/table/SwitchRow.svelte';
  import NumberStepperRow from '../ui/table/NumberStepperRow.svelte';
  import TableGroup from '../ui/table/TableGroup.svelte';
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
    contextLength = '10',
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



  // function handleSave() {
  //   // 转换回字符串格式，保持与原接口兼容
  //   const settingsToSave = {
  //     temperature: currentSettings.temperature.toString(),
  //     topP: currentSettings.topP.toString(),
  //     streamResponse: currentSettings.streamResponse,
  //     maxTokens: currentSettings.maxTokens.toString(),
  //     contextLength: currentSettings.contextLength.toString()
  //   }; 
  //   onSave?.(settingsToSave);
  // }

  // function handleReset() {
  //   currentSettings = { ...originalSettings };
  // }

  function handleDefault() {
    currentSettings = {
      temperature: 0.7,
      topP: 0.9,
      streamResponse: true,
      maxTokens: 2048,
      contextLength: 10
    };
  }
</script>

<div class="flex-1 p-0 space-y-6">

  <!-- 参数设置 -->
  <TableGroup>
    <!-- Temperature -->
    <LabeledSliderRow 
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

    <!-- Top-P -->
    <LabeledSliderRow 
      label="Top-p"
      bind:value={currentSettings.topP}
      min={0}
      max={1.0}
      step={0.1}
      leftLabel="聚焦"
      rightLabel="多样"
      scaleMarks={[
        { value: 0, position: 0 },
        { value: 0.5, position: 50 },
        { value: 1.0, position: 100 }
      ]}
      description=""
    />

    <!-- 流式输出 -->
    <SwitchRow 
      label="流式输出"
      bind:checked={currentSettings.streamResponse}
    />

    
  </TableGroup>

  <TableGroup>
    <!-- 最大 Token 数 -->
    <NumberStepperRow 
      label="最大输出长度"
      bind:value={currentSettings.maxTokens}
      defaultValue={0}
      placeholder="默认"
      min={100}
      max={10000000}
      step={100}
    />

    <!-- 上下文长度 -->
    <NumberStepperRow 
      label="上下文数"
      bind:value={currentSettings.contextLength}
      defaultValue={10}
      placeholder="默认"
      min={0}
      max={9999}
      step={1}
    />
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
  </div>
</div>
