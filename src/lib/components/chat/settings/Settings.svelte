<script lang="ts">
  import Drawer from "../../ui/Drawer.svelte";
  import PromptSettings from "./SettingsPrompt.svelte";
  import ModelSettings from "./SettingsModel.svelte";
  import McpSettings from "./SettingsMcp.svelte";

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();
</script>

<Drawer {open} title="聊天设置" {onClose}>
  <div class="flex flex-col gap-6 px-6 py-6 w-[420px] max-w-full">
    <section class="space-y-3">
      <ModelSettings variant="selection" />
    </section>

    <section class="space-y-3">
      <PromptSettings />
    </section>

    <section class="space-y-3">
      <h2
        class="text-xs font-semibold uppercase tracking-wide text-base-content/60"
      >
        模型参数
      </h2>
      <ModelSettings variant="parameters" />
    </section>

    <section class="space-y-3">
      <h2
        class="text-xs font-semibold uppercase tracking-wide text-base-content/60"
      >
        工具
      </h2>
      <McpSettings />
    </section>
  </div>
</Drawer>
