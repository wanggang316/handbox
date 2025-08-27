<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { X, Search, Check, Settings2, Trash2 } from "@lucide/svelte";
  import type { Model } from "$lib/types/provider";
    import IconButton from "../ui/IconButton.svelte";
    import { TableGroup, TableBaseRow } from "../ui/table";
    import RoundButton from "../ui/RoundButton.svelte";
    import Modal from "../ui/Modal.svelte";

  export let open = false;
  export let providerId: string;

  const dispatch = createEventDispatcher<{
    close: void;
    confirm: { selectedModels: string[] };
  }>();

  let searchQuery = "";
  let selectedModels = new Set<string>();
  let isLoading = false;
  
  // Modal 引用
  let modalRef: Modal;

  // 模拟模型数据 - 在实际实现中应该从props传入或API获取
  const mockModels: { provider: string; models: Model[] }[] = [
    {
      provider: "OpenAI",
      models: [
        {
          id: "gpt-5-chat",
          name: "OpenAI: GPT-5 Chat",
          provider: "openai",
          enabled: true,
          supportedFeatures: ["text", "vision", "function-calling"],
        },
        {
          id: "gpt-oss-20b",
          name: "OpenAI: gpt-oss-20b (free)",
          provider: "openai",
          enabled: true,
          supportedFeatures: ["text"],
        },
        {
          id: "gpt-5-mini",
          name: "OpenAI: GPT-5 Mini",
          provider: "openai",
          enabled: false,
          supportedFeatures: ["text", "function-calling"],
        },
        {
          id: "gpt-5-chat",
          name: "OpenAI: GPT-5 Chat",
          provider: "openai",
          enabled: true,
          supportedFeatures: ["text", "vision", "function-calling"],
        },
        {
          id: "gpt-oss-20b",
          name: "OpenAI: gpt-oss-20b (free)",
          provider: "openai",
          enabled: true,
          supportedFeatures: ["text"],
        },
        {
          id: "gpt-5-mini",
          name: "OpenAI: GPT-5 Mini",
          provider: "openai",
          enabled: false,
          supportedFeatures: ["text", "function-calling"],
        }
      ],
    },
    {
      provider: "Google",
      models: [
        {
          id: "gemini-2.5-flash-lite",
          name: "Google: Gemini 2.5 Flash Lite",
          provider: "google",
          enabled: false,
          supportedFeatures: ["text", "vision"],
        },
        {
          id: "gemini-2.5-flash",
          name: "Google: Gemini 2.5 Flash",
          provider: "google",
          enabled: false,
          supportedFeatures: ["text", "vision", "function-calling"],
        },
        {
          id: "gemini-2.5-pro",
          name: "Google: Gemini 2.5 Pro",
          provider: "google",
          enabled: true,
          supportedFeatures: [
            "text",
            "vision",
            "function-calling",
            "reasoning",
          ],
        },
        {
          id: "gemini-2.5-flash-lite",
          name: "Google: Gemini 2.5 Flash Lite",
          provider: "google",
          enabled: false,
          supportedFeatures: ["text", "vision"],
        },
        {
          id: "gemini-2.5-flash",
          name: "Google: Gemini 2.5 Flash",
          provider: "google",
          enabled: false,
          supportedFeatures: ["text", "vision", "function-calling"],
        },
        {
          id: "gemini-2.5-pro",
          name: "Google: Gemini 2.5 Pro",
          provider: "google",
          enabled: true,
          supportedFeatures: [
            "text",
            "vision",
            "function-calling",
            "reasoning",
          ],
        }
      ],
    },
  ];

  // 过滤模型
  $: filteredModels = mockModels
    .map((group) => ({
      ...group,
      models: group.models.filter(
        (model) =>
          model.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
          model.id.toLowerCase().includes(searchQuery.toLowerCase()),
      ),
    }))
    .filter((group) => group.models.length > 0);

  // 初始化已选中的模型
  $: {
    // 将已启用的模型添加到选中列表
    for (const group of mockModels) {
      for (const model of group.models) {
        if (model.enabled) {
          selectedModels.add(model.id);
        }
      }
    }
  }

  function toggleModel(modelId: string) {
    if (selectedModels.has(modelId)) {
      selectedModels.delete(modelId);
    } else {
      selectedModels.add(modelId);
    }
    selectedModels = selectedModels; // 触发响应式更新
  }

  function handleClose() {
    modalRef?.handleClose();
  }
  
  function onModalClose() {
    dispatch("close");
  }

  async function handleConfirm() {
    console.log("handleConfirm", selectedModels);
    isLoading = true;
    try {
      dispatch("confirm", {
        selectedModels: Array.from(selectedModels),
      });
      modalRef?.handleClose();
    } finally {
      isLoading = false;
    }
  }
</script>

<Modal bind:this={modalRef} {open} onClose={onModalClose} showCloseButton={false}>
  <!-- 弹窗容器 -->
  <div
    class="min-w-lg max-w-xl h-[80vh] overflow-hidden flex flex-col"
  >
    <!-- 头部 -->
    <div
      class="flex items-center justify-between gap-4 px-6 py-4"
    >
      <h2 class="font-normal text-text-primary">选择要使用模型</h2>

      <div class="relative">
        <Search
          class="absolute left-3 top-1/2 transform -translate-y-1/2 w-4 h-4 text-slate-400"
        />
        <input
          type="text"
          placeholder="搜索模型..."
          bind:value={searchQuery}
          class="w-full pl-10 pr-4 py-1 bg-slate-100 border-0 rounded-lg focus:ring-2 focus:ring-blue-500 focus:bg-white transition-colors"
        />
      </div>
    </div>

    <!-- 模型列表 -->
    <div class="flex-1 overflow-y-auto min-h-0 px-6 py-2">
      {#each filteredModels as group}
        <TableGroup title={group.provider}>
          {#each group.models as model}
            <TableBaseRow label={model.name} py="2">
              <input type="checkbox" bind:checked={model.enabled} on:change={() => toggleModel(model.id)} />
            </TableBaseRow>
          {/each}
        </TableGroup>
      {/each}
    </div>

    <!-- 底部按钮 -->
    <div
      class="flex items-center justify-end gap-3 px-6 py-3"
    >

    <RoundButton 
      customClass="w-18"
      label="取消" 
      bgColor="bg-gray-200"
      textColor="text-gray-600"
      hoverColor="hover:text-gray-800"
      on:click={handleClose} 
    ></RoundButton>
    <RoundButton 
      customClass="w-18"
      label="确认" 
      on:click={handleConfirm} 
      disabled={isLoading || selectedModels.size === 0}
    ></RoundButton>
    
    </div>
  </div>
</Modal>