<script lang="ts">
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import DropDown from "$lib/components/ui/DropDown.svelte";
  import { Plus, Send } from "@lucide/svelte";
  import IconButton from "../ui/IconButton.svelte";

  interface Props {
    messageInput?: string;
    selectedModel?: string;
    models?: string[];
    onSendMessage?: (message: string) => void;
    onModelChange?: (model: string) => void;
  }
  
  let { 
    messageInput = $bindable(''),
    selectedModel = $bindable('DeepSeek R1'),
    models = ['DeepSeek R1', 'Claude 3.5 Sonnet', 'GPT-4', 'Gemini Pro'],
    onSendMessage = (message: string) => console.log('Sending message:', message),
    onModelChange = (model: string) => console.log('Model changed to:', model)
  }: Props = $props();

  let textareaRef: HTMLTextAreaElement;

  // 将 models 数组转换为 DropDown 组件需要的格式
  const modelOptions = $derived(models.map(model => ({
    value: model,
    label: model
  })));

  function handleModelSelect(value: string) {
    selectedModel = value;
    onModelChange(value);
  }

  // 自动调整 textarea 高度
  function adjustTextareaHeight() {
    if (textareaRef) {
      textareaRef.style.height = 'auto';
      const scrollHeight = textareaRef.scrollHeight;
      const maxHeight = 300;
      textareaRef.style.height = Math.min(scrollHeight, maxHeight) + 'px';
    }
  }

  // 监听 messageInput 变化，自动调整高度
  $effect(() => {
    if (messageInput !== undefined) {
      adjustTextareaHeight();
    }
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      sendMessage();
    }
  }

  function sendMessage() {
    if (!messageInput.trim()) return;
    onSendMessage(messageInput);
    messageInput = '';
  }

  function handleAddAttachment() {
    console.log('添加附加');
  }
</script>

<div class="flex flex-col bg-[#f7f7f7] rounded-xl border border-[#ebeaea] max-h-[300px]">
  <textarea
    bind:this={textareaRef}
    bind:value={messageInput}
    placeholder="在这里输入消息，按 Enter 发送"
    onkeydown={handleKeydown}
    oninput={adjustTextareaHeight}
    rows="1"
    class="bg-transparent text-[14px] text-[#7e7e7f] p-4 outline-none resize-none w-full min-h-[48px] max-h-[200px] overflow-y-auto"
  ></textarea>

  <div class="flex flex-row justify-between items-center px-4 pt-0 pb-2">
    <!-- 左侧：添加按钮 -->
    <IconButton
      icon={Plus}
      ariaLabel="添加附加"
      on:click={handleAddAttachment}
    />

    <!-- 右侧：模型选择和发送按钮 -->
    <div class="flex items-center gap-3">
      <DropDown
        options={modelOptions}
        bind:selectedValue={selectedModel}
        placeholder="选择模型"
        position="top"
        align="right"
        onSelect={handleModelSelect}
      />

      <CircleButton
        icon={Send}
        ariaLabel="发送"
        on:click={sendMessage}
      />
    </div>
  </div>
  
</div>
