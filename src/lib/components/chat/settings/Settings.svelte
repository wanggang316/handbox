<script lang="ts">
  import Drawer from '../../ui/Drawer.svelte';
  import PromptSettings from './SettingsPrompt.svelte';
  import ModelSettings from './SettingsModel.svelte';
  import McpSettings from './SettingsMcp.svelte';
  import { MessageSquare, Settings as SettingsIcon, Server } from '@lucide/svelte';

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();
</script>

<Drawer {open} title="聊天设置" {onClose}>
  <div class="flex flex-col gap-8 px-6 py-6">
    <!-- 提示词配置组 -->
    <section class="flex flex-col gap-4">
      <div class="flex items-center gap-2">
        <MessageSquare size={18} class="text-primary" />
        <h3 class="text-base font-semibold text-base-content">提示词</h3>
      </div>
      <div class="pl-7">
        <PromptSettings />
      </div>
    </section>

    <!-- 模型参数配置组 -->
    <section class="flex flex-col gap-4">
      <div class="flex items-center gap-2">
        <SettingsIcon size={18} class="text-primary" />
        <h3 class="text-base font-semibold text-base-content">模型参数</h3>
      </div>
      <div class="pl-7">
        <ModelSettings />
      </div>
    </section>

    <!-- MCP 服务器配置组 -->
    <section class="flex flex-col gap-4">
      <div class="flex items-center gap-2">
        <Server size={18} class="text-primary" />
        <h3 class="text-base font-semibold text-base-content">MCP 服务器</h3>
      </div>
      <div class="pl-7">
        <McpSettings />
      </div>
    </section>
  </div>
</Drawer>