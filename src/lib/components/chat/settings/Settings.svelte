<script lang="ts">
  import Drawer from "../../ui/Drawer.svelte";
  import PromptSettings from "./Prompt.svelte";
  import ModelSelection from "./ModelSelection.svelte";
  import ModelParameters from "./ModelParameters.svelte";
  import McpSettings from "./Tools.svelte";

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();
</script>

<Drawer {open} title="聊天设置" {onClose}>
  <div class="flex flex-col gap-6 px-4 py-6 w-[360px]">
    <ModelSelection />
    <PromptSettings />
    <ModelParameters />
    <McpSettings />
  </div>
</Drawer>
