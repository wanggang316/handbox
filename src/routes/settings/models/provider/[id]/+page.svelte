<script lang="ts">
  import { page } from "$app/stores";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import {
    Eye,
    EyeOff,
    TestTube,
    RotateCw,
    Settings,
    Trash2,
    ChevronLeft,
    Edit,
    RefreshCw,
    Settings2,
  } from "@lucide/svelte";
  import ModelSelectModal from "$lib/components/settings/ModelSelectModal.svelte";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import TableBaseRow from "$lib/components/ui/table/TableBaseRow.svelte";
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import TextRow from "$lib/components/ui/table/TextRow.svelte";
  import Button from "$lib/components/ui/Button.svelte";

  let providerId = "";
  let isLoading = false;
  let showModelsModal = false;
  let showDeleteConfirm = false;
  let showApiKey = false;

  // 配置表单
  let formData = {
    baseUrl: "https://api.openai.com/v1",
    apiKey: "",
    enabled: true,
  };

  // 模拟数据
  const mockProvider = {
    name: "OpenAI",
    type: "openai",
    iconSrc: "/logo-openai.png",
    enabled: true,
  };

  const mockModels = {
    "OpenAI": [
      { name: "O3", id: "o3", enabled: true },
      { name: "O3 Mini", id: "o3-mini", enabled: true },
      { name: "O1", id: "o1", enabled: false },
      { name: "O1 Mini", id: "o1-mini", enabled: false },
      { name: "O1 Pro", id: "o1-pro", enabled: false },
      { name: "O1 Pro Mini", id: "o1-pro-mini", enabled: false },
      { name: "O1 Pro Max", id: "o1-pro-max", enabled: false },
      { name: "O1 Pro Max Mini", id: "o1-pro-max-mini", enabled: false },
    ],
    "Google": [
      { name: "Gemini 2.5 Flash", id: "gemini-2.5-flash", enabled: false },
      { name: "Gemini 2.5 Pro", id: "gemini-2.5-pro", enabled: false },
      { name: "Gemini 2.0 Flash", id: "gemini-2.0-flash", enabled: false },
    ],
  };

  onMount(() => {
    providerId = $page.params.id || "";
    loadProvider();
  });

  async function loadProvider() {
    // TODO: 从API加载供应商数据
    console.log("Loading provider:", providerId);
  }

  async function handleProbe() {
    if (!mockProvider) return;
    isLoading = true;
    try {
      // TODO: 实现探活检测
      await new Promise((resolve) => setTimeout(resolve, 1500));
      console.log("Probing provider:", mockProvider.name);
    } catch (error) {
      console.error("Probe failed:", error);
    } finally {
      isLoading = false;
    }
  }

  async function handleSave() {
    if (!mockProvider) return;
    isLoading = true;
    try {
      // TODO: 保存配置
      await new Promise((resolve) => setTimeout(resolve, 1000));
      console.log("Saving provider config:", formData);
    } catch (error) {
      console.error("Save failed:", error);
    } finally {
      isLoading = false;
    }
  }

  async function handleFetchModels() {
    console.log("handleFetchModels")
    if (!mockProvider) return;
    showModelsModal = true;
  }

  function handleModelsConfirm(
    event: CustomEvent<{ selectedModels: string[] }>,
  ) {
    const { selectedModels } = event.detail;
    console.log("Selected models:", selectedModels);
    showModelsModal = false;
  }

  function handleCloseModels() {
    showModelsModal = false;
  }

  async function handleDelete() {
    if (!mockProvider) return;
    showDeleteConfirm = true;
  }

  async function confirmDelete() {
    if (!mockProvider) return;
    isLoading = true;
    try {
      await new Promise((resolve) => setTimeout(resolve, 1000));
      console.log("Deleting provider:", mockProvider.name);
      goto("/settings/models");
    } catch (error) {
      console.error("Delete failed:", error);
    } finally {
      isLoading = false;
      showDeleteConfirm = false;
    }
  }

  function handleBack() {
    goto("/settings/models");
  }

  function handleRefreshModels(e: CustomEvent<any>): void {
    showModelsModal = true
  }

  function handleTestKey(): void {
    throw new Error("Function not implemented.");
  }

  function handleConfigModel(e: CustomEvent<any>): void {
    throw new Error("Function not implemented.");
  }

  function handleRemoveModel(e: CustomEvent<any>): void {
    throw new Error("Function not implemented.");
  }
</script>

{#snippet iconSnippet()}
  <img
    src={mockProvider.iconSrc}
    alt="{mockProvider.name} logo"
    class="w-6 h-6 object-contain"
  />
{/snippet}

<!-- 粘性导航栏 - 在右侧主体区域内固定 -->
<div class="flex flex-col h-screen">
  <header class=" text-white py-2 px-4 flex-shrink-0">
    <CircleButton
      icon={ChevronLeft}
      iconSize={22}
      ariaLabel="返回"
      size="w-8 h-8"
      bgColor="bg-bg-secondary"
      hoverColor="hover:bg-bg-hover"
      textColor="text-text-primary"
      customClass="hover:text-text-secondary z-10004 relative"
      on:click={handleBack}
    />
  </header>

  <!-- 主要内容区域 -->
  <main class="flex-grow overflow-y-auto p-6 pr-8">
    <TableBaseRow label={mockProvider.name} icon={iconSnippet}>
      <div class="flex flex-row items-center gap-4">
        <IconButton
          icon={Edit}
          on:click={() => (formData.enabled = !formData.enabled)}
        />

        <IconButton
          icon={Trash2}
          on:click={() => (formData.enabled = !formData.enabled)}
        />

        <Toggle checked={formData.enabled} />
      </div>
    </TableBaseRow>

    <div class="flex items-center justify-end">
      <Button on:click={handleTestKey} variant="clear" size="sm">
        <RefreshCw size={14} />
        检测
      </Button>
    </div>

    <TableGroup>
      <TextRow
        layout="vertical"
        isPassword
        label="API Key"
        value={formData.apiKey}
      />
      <TextRow layout="vertical" label="Base URL" value={formData.baseUrl} />
    </TableGroup>

    <div class="flex items-center mt-6">
      <div class="flex-1 text-text-primary text-base mx-2">模型列表</div>
      <Button on:click={handleFetchModels} variant="clear" size="sm">
        <RefreshCw size={14} />
        获取模型列表
      </Button>
    </div>

    {#each Object.keys(mockModels) as key}
      <TableGroup title={key}>
        {#each mockModels[key as keyof typeof mockModels] as model}
          <TableBaseRow label={model.name} py="2">
            <div class="flex flex-row items-center gap-2">
              <IconButton
                icon={Settings2}
                iconSize={16}
                on:click={handleConfigModel}
              />
              <IconButton
                icon={Trash2}
                iconSize={16}
                on:click={handleRemoveModel}
              />
            </div>
          </TableBaseRow>
        {/each}
      </TableGroup>
    {/each}

  </main>
</div>

<!-- 模型选择弹窗 -->
{#if showModelsModal}
  <ModelSelectModal
    {providerId}
    on:close={handleCloseModels}
    on:confirm={handleModelsConfirm}
  />
{/if}

<!-- 删除确认弹窗 -->
{#if showDeleteConfirm}
  <div
    class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
  >
    <div class="bg-white rounded-2xl p-6 max-w-md mx-4">
      <h3 class="text-lg font-semibold text-slate-900 mb-2">确认删除</h3>
      <p class="text-slate-600 mb-6">
        确定要删除该供应商吗？此操作不可撤销，所有相关配置和数据都将被清除。
      </p>

      <div class="flex items-center justify-end gap-3">
        <button
          on:click={() => (showDeleteConfirm = false)}
          class="px-4 py-2 text-slate-600 bg-slate-100 rounded-lg hover:bg-slate-200 transition-colors"
        >
          取消
        </button>
        <button
          on:click={confirmDelete}
          disabled={isLoading}
          class="flex items-center gap-2 px-4 py-2 bg-red-600 text-white rounded-lg hover:bg-red-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {#if isLoading}
            <RotateCw class="w-4 h-4 animate-spin" />
            <span>删除中...</span>
          {:else}
            <Trash2 class="w-4 h-4" />
            <span>确认删除</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}
