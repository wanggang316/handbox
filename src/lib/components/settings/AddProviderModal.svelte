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
    provider_type: "custom-openai" as const,
  };

  let isLoading = false;
  let errors: Record<string, string> = {};
  let showDropdown = false;
  
  // Modal 引用
  let modalRef: Modal;

  const providerTypes = [
    { value: "custom-openai", label: "OpenAI 兼容", icon: "🤖" },
    { value: "custom-anthropic", label: "Anthropic 兼容", icon: "🧠" },
  ];

  function validate() {
    errors = {};

    if (!formData.name.trim()) {
      errors.name = "请输入供应商名称";
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
        base_url: '',
        api_key: '',
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
  }

  $: selectedType = providerTypes.find(
    (t) => t.value === formData.provider_type,
  );
</script>

<Modal bind:this={modalRef} {open} onClose={onModalClose} showCloseButton={false}>
  <!-- 弹窗容器 -->
  <div
    class="bg-white w-full max-w-md max-h-[80vh] overflow-hidden flex flex-col"
  >
    <!-- 头部 -->
    <div class="flex items-center justify-between px-6 py-4">
      <h2 class="font-normal text-text-primary">添加供应商</h2>
    </div>

    <div class="flex-1 min-h-0 px-6 py-2">
      <TableGroup>
        <TextRow label="供应商名称" bind:value={formData.name}></TextRow>
        <DropDownRow
          label="供应商类型"
          options={providerTypes}
          selectedValue={formData.provider_type}
          onSelect={selectProviderType}
        ></DropDownRow>
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
