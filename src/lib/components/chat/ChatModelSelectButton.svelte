<script lang="ts">
  import { ChevronsUpDown } from "@lucide/svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import ChatModelSelectModal from "$lib/components/chat/ChatModelSelectModal.svelte";
  import type { ModelWithProvider } from "$lib/types/provider";

  interface Props {
    selectedModel?: ModelWithProvider | null;
    onModelSelect?: (model: ModelWithProvider) => void;
    variant?: "primary" | "secondary" | "gray" | "danger" | "ghost" | "clear";
    size?: "sm" | "md";
    customClass?: string;
  }

  let {
    selectedModel = null,
    onModelSelect = () => {},
    variant = "clear",
    size = "sm",
    customClass = "",
  }: Props = $props();

  let open = $state(false);

  function handleSelect(model: ModelWithProvider) {
    onModelSelect(model);
    open = false;
  }
</script>

<Button {variant} {size} {customClass} onclick={() => (open = true)}>
  {selectedModel ? selectedModel.name : "选择模型"}
  <ChevronsUpDown size={14} />
</Button>

<ChatModelSelectModal
  bind:open
  {selectedModel}
  onModelSelect={handleSelect}
/>
