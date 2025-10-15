<script lang="ts">
  import { chatState, chatActions, currentChatModel } from "$lib/states/chat.svelte";
  import LabeledSliderRow from "../../ui/table/LabeledSliderRow.svelte";
  import SwitchRow from "../../ui/table/SwitchRow.svelte";
  import NumberStepperRow from "../../ui/table/NumberStepperRow.svelte";
  import TableGroup from "../../ui/table/TableGroup.svelte";
  import RoundButton from "../../ui/RoundButton.svelte";
  import type { ModelParameter } from "$lib/types/provider";

  type SaveStatus = "saved" | "saving" | "error";

  // 获取模型参数信息
  const getModelParameters = (): ModelParameter[] => {
    const { model } = currentChatModel();
    return model?.parameters || [];
  };

  // 检查参数是否被模型支持
  const isParameterSupported = (paramName: string): boolean => {
    const params = getModelParameters();
    return params.some(p => p.name === paramName);
  };

  // 获取参数的默认值
  const getParameterDefault = (paramName: string, fallback: number): number => {
    const params = getModelParameters();
    const param = params.find(p => p.name === paramName);
    if (param?.default !== undefined && param.default !== null) {
      return Number(param.default);
    }
    return fallback;
  };

  // 获取当前聊天的设置，如果没有则使用默认值（考虑模型参数配置）
  const getInitialSettings = () => ({
    temperature: chatState.currentChat?.temperature || getParameterDefault('temperature', 0.7),
    topP: chatState.currentChat?.topP || getParameterDefault('top_p', 1.0),
    streamResponse: chatState.currentChat?.stream ?? true,
    maxTokens: chatState.currentChat?.maxTokens || getParameterDefault('max_tokens', 4000),
    turnCount: chatState.currentChat?.turnCount || 5, // 对话回合数，默认值为 5
  });

  let currentSettings = $state(getInitialSettings());
  let originalSettings = $state(getInitialSettings());
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let saveStatus = $state<SaveStatus>("saved");

  // 监听 currentChat 变化，更新本地状态
  $effect(() => {
    const newSettings = getInitialSettings();
    currentSettings = { ...newSettings };
    originalSettings = { ...newSettings };
    saveStatus = "saved";
  });

  // 自动保存：防抖处理
  $effect(() => {
    const hasChanges =
      JSON.stringify(currentSettings) !== JSON.stringify(originalSettings);
    if (hasChanges) {
      saveStatus = "saving";
      if (saveTimer) clearTimeout(saveTimer);
      saveTimer = setTimeout(async () => {
        try {
          await chatActions.updateModelSettings({
            temperature: currentSettings.temperature,
            topP: currentSettings.topP,
            stream: currentSettings.streamResponse,
            maxTokens: currentSettings.maxTokens,
            turnCount: currentSettings.turnCount,
          });

          // 更新原始设置，表示已保存
          originalSettings = { ...currentSettings };
          saveStatus = "saved";
        } catch (error) {
          console.error("Failed to update model settings:", error);
          saveStatus = "error";
        }
      }, 500); // 500ms 防抖延迟
    }
  });

  function handleDefault() {
    currentSettings = {
      temperature: getParameterDefault('temperature', 0.7),
      topP: getParameterDefault('top_p', 1.0),
      streamResponse: true,
      maxTokens: getParameterDefault('max_tokens', 4000),
      turnCount: 5,
    };
  }

  // 响应式：当模型变化时，重新获取参数支持信息
  $effect(() => {
    // 触发重新计算 - 当模型改变时，参数的可见性会自动更新
    const { model } = currentChatModel();
    if (model) {
      // 当模型改变时，更新设置为模型的默认值（如果当前值与旧默认值相同）
      const newSettings = getInitialSettings();
      currentSettings = { ...newSettings };
      originalSettings = { ...newSettings };
    }
  });
</script>

<div class="flex-1 p-0 space-y-6">
  <!-- 参数设置 -->
  <TableGroup>
    <!-- Temperature - 根据模型配置显示/隐藏 -->
    {#if isParameterSupported('temperature')}
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
          { value: 2, position: 100 },
        ]}
        description=""
      />
    {/if}

    <!-- Top-P - 根据模型配置显示/隐藏 -->
    {#if isParameterSupported('top_p')}
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
          { value: 1.0, position: 100 },
        ]}
        description=""
      />
    {/if}

    <!-- 流式输出 -->
    <SwitchRow label="流式输出" bind:checked={currentSettings.streamResponse} />

    <!-- 最大 Token 数 - 根据模型配置显示/隐藏 -->
    {#if isParameterSupported('max_tokens')}
      <NumberStepperRow
        label="最大输出长度"
        bind:value={currentSettings.maxTokens}
        defaultValue={getParameterDefault('max_tokens', 4000)}
        placeholder="{getParameterDefault('max_tokens', 4000)} (默认)"
        min={100}
        max={10000000}
        step={100}
      />
    {/if}

    <!-- 对话轮数 -->
    <NumberStepperRow
      label="对话轮数"
      bind:value={currentSettings.turnCount}
      defaultValue={5}
      placeholder="5 (默认)"
      min={1}
      max={100}
      step={1}
    />
  </TableGroup>

  <!-- 操作按钮 -->
  <div class="flex gap-3 pt-4 items-center justify-between">
    <RoundButton
      customClass="w-24"
      label="恢复默认"
      bgColor="bg-base-200"
      textColor="text-base-content/80"
      hoverColor="hover:text-base-content"
      onclick={handleDefault}
    />
    {#if saveStatus !== "saved"}
      <div class="px-6 py-2">
        <div class="flex items-center gap-2">
          <span
            class="w-2 h-2 rounded-full {saveStatus === 'saving'
              ? 'bg-warning'
              : 'bg-error'}"
          ></span>
          <span class="text-xs text-base-content/70">
            {saveStatus === "saving" ? "保存中..." : "保存失败"}
          </span>
        </div>
      </div>
    {/if}
  </div>
</div>
