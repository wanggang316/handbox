<script lang="ts">
  import type { AddProviderRequest } from "$lib/types/provider";
  import { 
    getProviderConfig, 
    getProviderDropdownOptions, 
    providerActions, 
    providerState, 
    providerStateActions 
  } from "$lib/states/provider.svelte";
  import TableGroup from "../ui/table/TableGroup.svelte";
  import TextRow from "../ui/table/TextRow.svelte";
  import SelectRow from "../ui/table/SelectRow.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Modal from "../ui/Modal.svelte";
  import { toastActions } from "$lib/states/toast.svelte";
  import { showAppError } from "$lib/utils";
  import { t } from "$lib/i18n";

  const { open = false, onClose } = $props<{
    open?: boolean;
    onClose?: () => void;
  }>();
  
  const editProvider = $derived(providerState.editingProvider);
  const isEditMode = $derived(editProvider !== null);

  // Baseline for change detection in edit mode.
  let originalData = $state({
    name: "",
    provider_type: "",
    base_url: "",
    api_key: "",
  });

  let formData = $state({
    name: "",
    provider_type: "openai",
    base_url: "",
    api_key: "",
  });

  let isLoading = $state(false);
  let errors = $state<Record<string, string>>({});

  const canSave = $derived(
    !isEditMode 
      ? // create mode: all required fields present
        !!(formData.name?.trim() && 
           formData.provider_type?.trim() && 
           formData.base_url?.trim() && 
           formData.api_key?.trim())
      : // edit mode: at least one field changed
        (formData.name !== originalData.name ||
         formData.provider_type !== originalData.provider_type ||
         formData.base_url !== originalData.base_url ||
         formData.api_key !== originalData.api_key)
  );
  
  let modalRef: Modal;

  const providerOptions = $derived(() => {
    const groups = getProviderDropdownOptions();
    return groups.flatMap(group => group.options);
  });

  function handleError(error: unknown) {
    console.error("Operation failed:", error);
    showAppError(error, {
      requiresAcknowledgement: true,
      title: t("provider.configErrorTitle"),
      fallbackMessage: t("provider.operationFailed")
    });
  }

  function validate() {
    errors = {};

    if (!formData.name.trim()) {
      errors.name = t("provider.validateName");
    }

    if (!formData.base_url.trim()) {
      errors.base_url = t("provider.validateBaseUrl");
    }

    if (!formData.api_key.trim()) {
      errors.api_key = t("provider.validateApiKey");
    }

    return Object.keys(errors).length === 0;
  }

  function handleClose() {
    modalRef?.handleClose();
  }
  
  function onModalClose() {
    providerStateActions.endEditProvider();
    onClose?.();
  }

  async function handleConfirm() {
    if (!validate()) {
      console.log("errors", errors);
      return;
    } 

    isLoading = true;
    try {
      const config: AddProviderRequest = {
        name: formData.name,
        provider_type: formData.provider_type,
        base_url: formData.base_url,
        api_key: formData.api_key,
        enabled: true,
      };

      if (isEditMode && editProvider && editProvider.id) {
        console.log("Updating provider with config:", config);
        await providerActions.updateProvider(editProvider.id, config);
        console.log("Provider updated successfully");
        
        // Refresh the current provider's details (the model list may have changed).
        await providerStateActions.refreshCurrentProvider();
      } else {
        console.log("Creating provider with config:", config);
        const newProvider = await providerActions.createProvider(config);
        console.log("Provider created successfully:", newProvider);
      }
      toastActions.success(
        isEditMode ? t("provider.updateSuccess") : t("provider.createSuccess")
      );
      modalRef?.handleClose();
    } catch (error) {
      handleError(error);
    } finally {
      isLoading = false;
    }
  }

  function selectProviderType(type: string) {
    formData.provider_type = type;
    
    // Predefined provider types auto-fill name and base URL.
    const selectedProviderConfig = getProviderConfig(type);
    if (selectedProviderConfig) {
      formData.name = selectedProviderConfig.default_name;
      formData.base_url = selectedProviderConfig.default_base_url;
    } else {
      // Unknown type: clear the name only if it is empty or still a preset
      // default, preserving a user-typed custom name.
      const allGroups = getProviderDropdownOptions();
      const currentConfigNames = allGroups.flatMap(group => group.options.map(opt => opt.label));
      if (formData.name === '' || currentConfigNames.includes(formData.name)) {
        formData.name = '';
      }
      formData.base_url = '';
    }
  }

  $effect(() => {
    if (open) {
      initializeFormData();
    } else {
      formData = {
        name: "",
        provider_type: "openai",
        base_url: "",
        api_key: "",
      };
      errors = {};
      providerStateActions.endEditProvider();
    }
  });
  
  function initializeFormData() {
    if (isEditMode && editProvider) {
      originalData = {
        name: editProvider.name,
        provider_type: editProvider.provider_type,
        base_url: editProvider.base_url,
        api_key: editProvider.api_key
      };
      formData = {
        name: editProvider.name,
        provider_type: editProvider.provider_type,
        base_url: editProvider.base_url,
        api_key: editProvider.api_key
      };
      console.log("editProvider", editProvider);
    } else if (!isEditMode && formData.provider_type === "openai" && formData.name === "") {
      const defaultProviderConfig = getProviderConfig("openai");
      if (defaultProviderConfig) {
        formData.name = defaultProviderConfig.default_name;
        formData.base_url = defaultProviderConfig.default_base_url;
      }
      originalData = {
        name: "",
        provider_type: "openai",
        base_url: "",
        api_key: "",
      };
    }
  }
</script>

<Modal bind:this={modalRef} {open} onClose={onModalClose} showCloseButton={false}>
  <!-- Surface and border are provided by Modal.svelte -->
  <div class="w-md max-w-md max-h-[80vh] flex flex-col">
    <div class="flex items-center justify-between px-5 py-3.5">
      <h2 class="text-base font-medium tracking-tight text-base-content">{isEditMode ? t("provider.editProviderTitle") : t("provider.addProviderTitle")}</h2>
    </div>

    <div class="flex-1 min-h-0 px-5 py-2 space-y-3">
      <TableGroup>
        <SelectRow
          label={t("provider.providerType")}
          options={providerOptions()}
          selectedValue={formData.provider_type}
          onSelect={selectProviderType}
        ></SelectRow>
        <TextRow label={t("provider.providerName")} bind:value={formData.name}></TextRow>
      </TableGroup>
      <TableGroup>
        <TextRow label="Base URL" bind:value={formData.base_url}></TextRow>
        <TextRow label="API Key" bind:value={formData.api_key} isPassword={true}></TextRow>
      </TableGroup>
    </div>

    <div class="flex items-center justify-end gap-3 px-5 py-3">
      <Button
        class="w-18"
        size="lg"
        variant="secondary"
        onclick={handleClose}
      >{t("common.cancel")}</Button>
      <Button
        class="w-18"
        size="lg"
        onclick={handleConfirm}
        disabled={isLoading || !canSave}
        loading={isLoading}
      >{isEditMode ? t("common.save") : t("provider.confirm")}</Button>
    </div>
  </div>
</Modal>
