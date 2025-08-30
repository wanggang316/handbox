<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import type { ProviderConfig } from "$lib/types/provider";
  import { preProviders } from "$lib/states/provider.svelte";
  import TableGroup from "../ui/table/TableGroup.svelte";
  import TextRow from "../ui/table/TextRow.svelte";
  import DropDownRow from "../ui/table/DropDownRow.svelte";
  import RoundButton from "../ui/RoundButton.svelte";
  import Modal from "../ui/Modal.svelte";

  export let open = false;
  
  const dispatch = createEventDispatcher<{
    close: void;
    confirm: ProviderConfig;
  }>();

  let formData = {
    name: "",
    provider_type: "openai" as const,
    base_url: "",
    api_key: "",
  };

  let isLoading = false;
  let errors: Record<string, string> = {};
  let showDropdown = false;
  
  // Modal 引用
  let modalRef: Modal;

  // 将预定义供应商转换为选项格式
  const preProviderOptions = preProviders.map(provider => ({
    value: provider.provider_type,
    label: provider.name,
    icon: provider.iconSrc
  }));

  // 自定义供应商类型选项
  const customProviderOptions = [
    { value: "custom-openai", label: "自定义 OpenAI 兼容", icon: "🤖" },
    { value: "custom-anthropic", label: "自定义 Anthropic 兼容", icon: "🧠" },
  ];

  // 分组供应商类型
  const providerGroups = [
    {
      title: "预定义供应商",
      options: preProviderOptions
    },
    {
      title: "自定义供应商",
      options: customProviderOptions
    }
  ];

  function validate() {
    errors = {};

    if (!formData.name.trim()) {
      errors.name = "请输入供应商名称";
    }

    if (!formData.base_url.trim()) {
      errors.base_url = "请输入 Base URL";
    }

    if (!formData.api_key.trim()) {
      errors.api_key = "请输入 API Key";
    }

    return Object.keys(errors).length === 0;
  }

  function handleClose() {
    modalRef?.handleClose();
  }
  
  function onModalClose() {
    dispatch("close");
  }

  async function handleConfirm() {
    if (!validate()) {
      console.log("errors", errors);
      return;
    } 

    isLoading = true;
    try {
      const config: ProviderConfig = {
        name: formData.name,
        provider_type: formData.provider_type,
        base_url: formData.base_url,
        api_key: formData.api_key,
        enabled: false,
      };

      console.log("config", config);

      dispatch("confirm", config);
      modalRef?.handleClose();
    } catch (error) {
      console.error("Failed to create provider:", error);
    } finally {
      isLoading = false;
    }
  }

  function selectProviderType(type: string) {
    formData.provider_type = type as any;
    showDropdown = false;
    
    // 如果选择了预定义供应商，自动填充名称
    const selectedPreProvider = preProviders.find(p => p.provider_type === type);
    if (selectedPreProvider) {
      formData.name = selectedPreProvider.name;
      formData.base_url = selectedPreProvider.base_url_placeholder;
    } else {
      // 自定义供应商，清空名称让用户自己填写
      if (formData.name === '' || preProviders.some(p => p.name === formData.name)) {
        formData.name = '';
      }
      formData.base_url = '';
    }
  }

  // 初始化表单数据
  $: {
    if (formData.provider_type === "openai" && formData.name === "") {
      const defaultProvider = preProviders.find(p => p.provider_type === "openai");
      if (defaultProvider) {
        formData.name = defaultProvider.name;
        formData.base_url = defaultProvider.base_url_placeholder;
      }
    }
  }
</script>

<Modal bind:this={modalRef} {open} onClose={onModalClose} showCloseButton={false}>
  <!-- 弹窗容器 -->
  <div
    class="w-md max-w-md max-h-[80vh] flex flex-col"
  >
    <!-- 头部 -->
    <div class="flex items-center justify-between px-6 py-4">
      <h2 class="font-normal text-text-primary">添加供应商</h2>
    </div>

    <div class="flex-1 min-h-0 px-6 py-2 space-y-4">
      <TableGroup>
        <DropDownRow
          label="供应商类型"
          groups={providerGroups}
          selectedValue={formData.provider_type}
          onSelect={selectProviderType}
        ></DropDownRow>
        <TextRow label="供应商名称" bind:value={formData.name}></TextRow>
      </TableGroup>
      <TableGroup>
        <TextRow label="Base URL" bind:value={formData.base_url}></TextRow>
        <TextRow label="API Key" bind:value={formData.api_key} isPassword={true}></TextRow>
      </TableGroup>
    </div>

    <!-- 底部按钮 -->
    <div class="flex items-center justify-end gap-3 px-6 py-3">
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
        disabled={isLoading}
      ></RoundButton>
    </div>
  </div>
</Modal>
