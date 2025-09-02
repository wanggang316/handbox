<script lang="ts">
  import type { ProviderConfig, Provider } from "$lib/types/provider";
  import { 
    getProviderConfig, 
    getProviderDropdownOptions, 
    providerActions, 
    providerState, 
    providerStateActions 
  } from "$lib/states/provider.svelte";
  import TableGroup from "../ui/table/TableGroup.svelte";
  import TextRow from "../ui/table/TextRow.svelte";
  import DropDownRow from "../ui/table/DropDownRow.svelte";
  import RoundButton from "../ui/RoundButton.svelte";
  import Modal from "../ui/Modal.svelte";

  // 使用 $props() 替代 export let
  const { open = false, onClose } = $props<{
    open?: boolean;
    onClose?: () => void;
  }>();
  
  // 使用统一的状态管理
  const editProvider = $derived(providerState.editingProvider);
  const isEditMode = $derived(editProvider !== null);

  // 使用 $state 定义响应式状态
  let formData = $state({
    name: "",
    provider_type: "openai",
    base_url: "",
    api_key: "",
  });

  let isLoading = $state(false);
  let errors = $state<Record<string, string>>({});
  
  // Modal 引用
  let modalRef: Modal;

  // 使用统一的工具函数获取供应商分组
  const providerGroups = $derived(getProviderDropdownOptions());

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
    providerStateActions.endEditProvider();
    // 通知父组件关闭模态框
    onClose?.();
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
        enabled: true,
      };

      if (isEditMode && editProvider) {
        // 编辑模式：更新供应商
        console.log("Updating provider with config:", config);
        await providerActions.updateProvider(editProvider.id, config);
        console.log("Provider updated successfully");
        
        // 编辑模式：通知状态管理器更新当前供应商
        const updatedProvider: Provider = {
          ...editProvider,
          name: formData.name,
          provider_type: formData.provider_type,
          base_url: formData.base_url,
          api_key: formData.api_key,
        };
        providerStateActions.updateCurrentProvider(updatedProvider);
      } else {
        // 创建模式：创建新供应商
        console.log("Creating provider with config:", config);
        const newProvider = await providerActions.createProvider(config);
        console.log("Provider created successfully:", newProvider);
      }
      // 成功后触发关闭动画
      modalRef?.handleClose();
    } catch (error) {
      console.error(isEditMode ? "Failed to update provider:" : "Failed to create provider:", error);
      // 这里可以显示错误信息给用户
    } finally {
      isLoading = false;
    }
  }

  function selectProviderType(type: string) {
    formData.provider_type = type;
    
    // 如果选择了预定义供应商，自动填充名称
    const selectedProviderConfig = getProviderConfig(type);
    if (selectedProviderConfig) {
      formData.name = selectedProviderConfig.default_name;
      formData.base_url = selectedProviderConfig.default_base_url;
    } else {
      // 如果没有找到配置，清空名称让用户自己填写
      // 获取所有配置的默认名称
      const allGroups = getProviderDropdownOptions();
      const currentConfigNames = allGroups.flatMap(group => group.options.map(opt => opt.label));
      if (formData.name === '' || currentConfigNames.includes(formData.name)) {
        formData.name = '';
      }
      formData.base_url = '';
    }
  }

  // 使用 $effect 替代 $: 响应式语句
  // 当模态框打开时初始化表单数据
  $effect(() => {
    if (open) {
      initializeFormData();
    } else {
      // 当模态框关闭时重置表单数据和状态
      formData = {
        name: "",
        provider_type: "openai",
        base_url: "",
        api_key: "",
      };
      errors = {};
      // 确保结束编辑状态
      providerStateActions.endEditProvider();
    }
  });
  
  // 抽取初始化逻辑为单独的函数
  function initializeFormData() {
    if (isEditMode && editProvider) {
      formData = {
        name: editProvider.name,
        provider_type: editProvider.provider_type,
        base_url: editProvider.base_url,
        api_key: editProvider.api_key
      };
      console.log("editProvider", editProvider);
    } else if (!isEditMode && formData.provider_type === "openai" && formData.name === "") {
      // 创建模式的默认初始化
      const defaultProviderConfig = getProviderConfig("openai");
      if (defaultProviderConfig) {
        formData.name = defaultProviderConfig.default_name;
        formData.base_url = defaultProviderConfig.default_base_url;
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
      <h2 class="font-normal text-text-primary">{isEditMode ? '编辑供应商' : '添加供应商'}</h2>
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
        label={isEditMode ? '保存' : '确认'}
        on:click={handleConfirm}
        disabled={isLoading}
      ></RoundButton>
    </div>
  </div>
</Modal>
