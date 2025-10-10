<script lang="ts">
  import { chatState, chatActions } from "$lib/states/chat.svelte";
  import LabeledSliderRow from "../../ui/table/LabeledSliderRow.svelte";
  import SwitchRow from "../../ui/table/SwitchRow.svelte";
  import NumberStepperRow from "../../ui/table/NumberStepperRow.svelte";
  import TableGroup from "../../ui/table/TableGroup.svelte";
  import RoundButton from "../../ui/RoundButton.svelte";

  type SaveStatus = "saved" | "saving" | "error";

  // 获取当前聊天的设置，如果没有则使用默认值
  const getInitialSettings = () => ({
    temperature: chatState.currentChat?.temperature || 0.7,
    topP: chatState.currentChat?.topP || 1.0,
    streamResponse: chatState.currentChat?.stream ?? true,
    maxTokens: chatState.currentChat?.maxTokens || 4000,
    contextLength: 10, // contextLength 目前不存储在 Chat 中，使用默认值
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
            contextLength: currentSettings.contextLength,
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
      temperature: 0.7,
      topP: 1.0,
      streamResponse: true,
      maxTokens: 4000,
      contextLength: 10,
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
        { value: 2, position: 100 },
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
        { value: 1.0, position: 100 },
      ]}
      description=""
    />

    <!-- 流式输出 -->
    <SwitchRow label="流式输出" bind:checked={currentSettings.streamResponse} />
  </TableGroup>

  <TableGroup>
    <!-- 最大 Token 数 -->
    <NumberStepperRow
      label="最大输出长度"
      bind:value={currentSettings.maxTokens}
      defaultValue={4000}
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
