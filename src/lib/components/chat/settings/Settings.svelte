<script lang="ts">
  import Drawer from "../../ui/Drawer.svelte";
  import PromptSettings from "./SettingsPrompt.svelte";
  import ModelSettings from "./SettingsModel.svelte";
  import McpSettings from "./SettingsMcp.svelte";
  import {
    MessageSquare,
    Settings as SettingsIcon,
    Server,
  } from "@lucide/svelte";

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();
</script>

<Drawer {open} title="聊天设置" {onClose}>
  <div class="flex flex-col gap-8 px-4 py-4 w-sm">
    <!-- 提示词配置组 -->
    <section class="flex flex-col gap-4">
      <div class="">
        <PromptSettings />
      </div>
    </section>

    <!-- 模型参数配置组 -->
    <section class="flex flex-col gap-4">
      <div class="">
        <ModelSettings />
      </div>
    </section>

    <!-- MCP 服务器配置组 -->
    <section class="flex flex-col gap-4">
      <div class="">
        <McpSettings />
      </div>
    </section>
  </div>
</Drawer>
