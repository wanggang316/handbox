<script lang="ts">
  import Modal from "$lib/components/ui/Modal.svelte";
  import RoundButton from "$lib/components/ui/RoundButton.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import TextRow from "$lib/components/ui/table/TextRow.svelte";
  import TextareaRow from "$lib/components/ui/table/TextareaRow.svelte";
  import SelectRow from "$lib/components/ui/table/SelectRow.svelte";
  import { showAppError } from "$lib/utils";
  import { t } from "$lib/i18n";
  import type {
    CreateMcpServerRequest,
    McpConnectionType,
    McpServer,
    UpdateMcpServerRequest,
  } from "$lib/types";
  interface EnvEntry {
    key: string;
    value: string;
  }

  interface HeaderEntry {
    key: string;
    value: string;
  }

  interface Props {
    open?: boolean;
    server?: McpServer | null;
    onClose?: () => void;
    onSave?: (data: {
      mode: "create" | "update";
      data: CreateMcpServerRequest | UpdateMcpServerRequest;
    }) => Promise<void>;
  }

  let {
    open = $bindable(false),
    server = $bindable<McpServer | null>(null),
    onClose,
    onSave,
  }: Props = $props();

  // 使用Svelte 5的$state替代传统状态
  let modalRef: Modal;
  let isSubmitting = $state(false);
  let errors = $state<Record<string, string>>({});
  let envEntries = $state<EnvEntry[]>([]);
  let headerEntries = $state<HeaderEntry[]>([]);

  interface FormState {
    name: string;
    displayName: string;
    description: string;
    connectionType: McpConnectionType;
    command: string;
    argsText: string;
    workingDir: string;
    endpoint: string;
    timeoutMs: string;
    enabled: boolean;
  }

  const EMPTY_FORM: FormState = {
    name: "",
    displayName: "",
    description: "",
    connectionType: "stdio",
    command: "",
    argsText: "",
    workingDir: "",
    endpoint: "",
    timeoutMs: "",
    enabled: true,
  };

  const BLANK_ENTRY = (): EnvEntry => ({ key: "", value: "" });
  const BLANK_HEADER = (): HeaderEntry => ({ key: "", value: "" });

  // 表单数据
  let formData = $state<FormState>({ ...EMPTY_FORM });

  // 检查是否为编辑模式
  const isEditMode = $derived(server !== null);

  // 检查是否可以保存
  const canSave = $derived(() => {
    const hasName = formData.name.trim();
    const hasValidConnection = formData.connectionType === 'stdio'
      ? formData.command.trim()
      : formData.endpoint.trim();
    return hasName && hasValidConnection && !isSubmitting;
  });

  function buildEnvEntries(source: Record<string, string>): EnvEntry[] {
    const entries = Object.entries(source).map(([key, value]) => ({ key, value }));
    return entries.length > 0 ? entries : [BLANK_ENTRY()];
  }

  function buildHeaderEntries(source: Record<string, string>): HeaderEntry[] {
    const entries = Object.entries(source).map(([key, value]) => ({ key, value }));
    return entries.length > 0 ? entries : [BLANK_HEADER()];
  }

  function initialiseForm(current: McpServer | null) {
    if (current) {
      formData = {
        name: current.name,
        displayName: current.displayName ?? "",
        description: current.description ?? "",
        connectionType: current.connectionType,
        command: current.command,
        argsText: current.args.join("\n"),
        workingDir: current.workingDir ?? "",
        endpoint: current.endpoint ?? "",
        timeoutMs: current.timeoutMs?.toString() ?? "",
        enabled: current.enabled,
      };
      envEntries = buildEnvEntries(current.env);
      headerEntries = buildHeaderEntries(current.headers);
    } else {
      formData = { ...EMPTY_FORM };
      envEntries = [BLANK_ENTRY()];
      headerEntries = [BLANK_HEADER()];
    }
    errors = {};
  }

  // 当弹窗打开或 server 变化时，重新初始化表单
  $effect(() => {
    if (open) {
      initialiseForm(server);
    }
  });

  function closeModal() {
    modalRef?.handleClose();
  }

  function onModalClose() {
    onClose?.();
  }

  // 环境变量操作
  function addEnvEntry() {
    envEntries = [...envEntries, BLANK_ENTRY()];
  }

  function removeEnvEntry(index: number) {
    envEntries = envEntries.length === 1
      ? [BLANK_ENTRY()]
      : envEntries.filter((_, idx) => idx !== index);
  }

  function updateEnvEntry(index: number, field: "key" | "value", value: string) {
    envEntries = envEntries.map((entry, idx) =>
      idx === index ? { ...entry, [field]: value } : entry
    );
  }

  // HTTP 头部操作
  function addHeaderEntry() {
    headerEntries = [...headerEntries, BLANK_HEADER()];
  }

  function removeHeaderEntry(index: number) {
    headerEntries = headerEntries.length === 1
      ? [BLANK_HEADER()]
      : headerEntries.filter((_, idx) => idx !== index);
  }

  function updateHeaderEntry(index: number, field: "key" | "value", value: string) {
    headerEntries = headerEntries.map((entry, idx) =>
      idx === index ? { ...entry, [field]: value } : entry
    );
  }

  function validate(): boolean {
    const nextErrors: Record<string, string> = {};

    // 验证名称
    if (!formData.name.trim()) {
      nextErrors.name = t("provider.validateMcpName");
    }

    // 验证连接配置
    if (formData.connectionType === 'stdio') {
      if (!formData.command.trim()) {
        nextErrors.command = t("provider.validateCommand");
      }
    } else {
      if (!formData.endpoint.trim()) {
        nextErrors.endpoint = t("provider.validateEndpoint");
      }
      // 验证超时时间
      if (formData.timeoutMs && isNaN(Number(formData.timeoutMs))) {
        nextErrors.timeoutMs = t("provider.validateTimeout");
      }
    }

    errors = nextErrors;
    return Object.keys(nextErrors).length === 0;
  }

  // 解析参数（支持换行或逗号分隔）
  function parseArgs(): string[] {
    return formData.argsText
      .split(/\r?\n|,/)
      .map(arg => arg.trim())
      .filter(Boolean);
  }

  // 解析环境变量
  function parseEnv(): Record<string, string> {
    return envEntries.reduce<Record<string, string>>((acc, entry) => {
      const key = entry.key.trim();
      if (key) acc[key] = entry.value;
      return acc;
    }, {});
  }

  // 解析 HTTP 头部
  function parseHeaders(): Record<string, string> {
    return headerEntries.reduce<Record<string, string>>((acc, entry) => {
      const key = entry.key.trim();
      if (key) acc[key] = entry.value;
      return acc;
    }, {});
  }

  async function handleConfirm() {
    if (!validate()) return;

    isSubmitting = true;

    try {
      if (server) {
        // 更新模式
        const updatePayload: UpdateMcpServerRequest = {
          name: formData.name.trim(),
          displayName: formData.displayName.trim() || undefined,
          description: formData.description.trim() || undefined,
          connectionType: formData.connectionType,
          enabled: formData.enabled,
        };

        if (formData.connectionType === 'stdio') {
          updatePayload.command = formData.command.trim();
          updatePayload.args = parseArgs();
          updatePayload.workingDir = formData.workingDir.trim() || undefined;
          updatePayload.env = parseEnv();
        } else {
          updatePayload.command = '';
          updatePayload.endpoint = formData.endpoint.trim() || undefined;
          updatePayload.headers = parseHeaders();
          updatePayload.timeoutMs = formData.timeoutMs ? Number(formData.timeoutMs) : undefined;
        }

        await onSave?.({ mode: "update", data: updatePayload });
      } else {
        // 创建模式
        const createPayload: CreateMcpServerRequest = {
          name: formData.name.trim(),
          displayName: formData.displayName.trim() || undefined,
          description: formData.description.trim() || undefined,
          connectionType: formData.connectionType,
          command: formData.connectionType === 'stdio' ? formData.command.trim() : '',
          enabled: formData.enabled,
        };

        if (formData.connectionType === 'stdio') {
          createPayload.args = parseArgs();
          createPayload.workingDir = formData.workingDir.trim() || undefined;
          createPayload.env = parseEnv();
        } else {
          createPayload.endpoint = formData.endpoint.trim() || undefined;
          createPayload.headers = parseHeaders();
          createPayload.timeoutMs = formData.timeoutMs ? Number(formData.timeoutMs) : undefined;
        }

        await onSave?.({ mode: "create", data: createPayload });
      }

      // 保存成功，关闭弹窗
      closeModal();
    } catch (error) {
      showAppError(error, {
        fallbackMessage: t("provider.saveFailed")
      });
    } finally {
      isSubmitting = false;
    }
  }
</script>

<Modal
  bind:this={modalRef}
  {open}
  onClose={onModalClose}
  showCloseButton={false}
>
  <!-- 弹窗容器 -->
  <div class="w-lg max-w-lg h-[90vh] max-h-[100vh] flex flex-col">
    <!-- 头部 -->
    <div class="flex items-center justify-between px-6 py-4">
      <h2 class="font-normal text-base-content">
        {isEditMode ? t("provider.editMcpTitle") : t("provider.addMcpTitle")}
      </h2>
      <div class="flex items-center gap-2">
        <Toggle bind:checked={formData.enabled} />
      </div>
    </div>

    <div class="flex-1 min-h-0 px-6 py-2 space-y-4 overflow-y-auto">
      <!-- 基本信息 -->
      <TableGroup>
        <TextRow
          label={t("provider.mcpName")}
          bind:value={formData.name}
          placeholder={t("provider.mcpNamePlaceholder")}
        />
        <TextRow
          label={t("provider.mcpDisplayName")}
          bind:value={formData.displayName}
          placeholder={t("provider.mcpDisplayNamePlaceholder")}
        />

        <SelectRow
          label={t("provider.connectionType")}
          bind:selectedValue={formData.connectionType}
          options={[
            { value: "stdio", label: t("provider.connectionStdio") },
            { value: "sse", label: t("provider.connectionSse") },
            { value: "http", label: t("provider.connectionHttp") }
          ]}
        />

        {#if formData.connectionType === 'stdio'}
          <TextRow
            label={t("provider.mcpCommand")}
            bind:value={formData.command}
            placeholder={t("provider.mcpCommandPlaceholder")}
          />
          <TextareaRow
            label={t("provider.mcpArgs")}
            bind:value={formData.argsText}
            placeholder={t("provider.mcpArgsPlaceholder")}
            rows={3}
          />
          <TextRow
            label={t("provider.mcpWorkingDir")}
            bind:value={formData.workingDir}
            placeholder={t("provider.optional")}
            layout="vertical"
          />
        {:else}
          <TextRow
            label={t("provider.mcpEndpoint")}
            bind:value={formData.endpoint}
            placeholder={t("provider.mcpEndpointPlaceholder")}
          />
          <TextRow
            label={t("provider.mcpTimeout")}
            bind:value={formData.timeoutMs}
            placeholder={t("provider.mcpTimeoutPlaceholder")}
          />
        {/if}
      </TableGroup>

      <!-- 环境变量 (只对 stdio 连接显示) -->
      {#if formData.connectionType === 'stdio'}
        <TableGroup>
          <div class="p-4 space-y-3">
            <div class="flex items-center justify-between">
              <span class="text-sm text-base-content/80">{t("provider.envVars")}</span>
              <button
                class="text-primary text-sm hover:text-primary/80"
                type="button"
                onclick={addEnvEntry}
              >
                {t("provider.addEntry")}
              </button>
            </div>

            <div class="space-y-2">
              {#each envEntries as entry, index (index)}
                <div class="grid grid-cols-[1fr_1fr_auto] gap-2 items-center">
                  <input
                    class="w-full px-3 py-2 text-sm bg-base-300 border border-[var(--hairline)] rounded-lg focus:border-primary"
                    placeholder={t("provider.envKeyPlaceholder")}
                    value={entry.key}
                    oninput={(e) =>
                      updateEnvEntry(index, "key", e.currentTarget.value)}
                  />
                  <input
                    class="w-full px-3 py-2 text-sm bg-base-300 border border-[var(--hairline)] rounded-lg focus:border-primary"
                    placeholder={t("provider.envValuePlaceholder")}
                    value={entry.value}
                    oninput={(e) =>
                      updateEnvEntry(index, "value", e.currentTarget.value)}
                  />
                  <button
                    class="text-error text-sm hover:text-error/80 px-2"
                    type="button"
                    onclick={() => removeEnvEntry(index)}
                  >
                    {t("common.delete")}
                  </button>
                </div>
              {/each}
            </div>
          </div>
        </TableGroup>
      {/if}

      <!-- HTTP 头部 (只对 SSE/HTTP 连接显示) -->
      {#if formData.connectionType === 'sse' || formData.connectionType === 'http'}
        <TableGroup>
          <div class="p-4 space-y-3">
            <div class="flex items-center justify-between">
              <span class="text-sm text-base-content/80">{t("provider.httpHeaders")}</span>
              <button
                class="text-primary text-sm hover:text-primary/80"
                type="button"
                onclick={addHeaderEntry}
              >
                {t("provider.addEntry")}
              </button>
            </div>

            <div class="space-y-2">
              {#each headerEntries as entry, index (index)}
                <div class="grid grid-cols-[1fr_1fr_auto] gap-2 items-center">
                  <input
                    class="w-full px-3 py-2 text-sm bg-base-300 border border-[var(--hairline)] rounded-lg focus:border-primary"
                    placeholder={t("provider.headerKeyPlaceholder")}
                    value={entry.key}
                    oninput={(e) =>
                      updateHeaderEntry(index, "key", e.currentTarget.value)}
                  />
                  <input
                    class="w-full px-3 py-2 text-sm bg-base-300 border border-[var(--hairline)] rounded-lg focus:border-primary"
                    placeholder={t("provider.headerValuePlaceholder")}
                    value={entry.value}
                    oninput={(e) =>
                      updateHeaderEntry(index, "value", e.currentTarget.value)}
                  />
                  <button
                    class="text-error text-sm hover:text-error/80 px-2"
                    type="button"
                    onclick={() => removeHeaderEntry(index)}
                  >
                    {t("common.delete")}
                  </button>
                </div>
              {/each}
            </div>
          </div>
        </TableGroup>
      {/if}

      <!-- 错误提示 -->
      {#if Object.keys(errors).length > 0}
        <div class="bg-error/10 border border-error/20 rounded-lg p-3">
          {#each Object.entries(errors) as [, message]}
            <p class="text-xs text-error">{message}</p>
          {/each}
        </div>
      {/if}
    </div>

    <!-- 底部按钮 -->
    <div class="flex items-center justify-end gap-3 px-6 py-3">
      <RoundButton
        customClass="w-18"
        label={t("common.cancel")}
        variant="secondary"
        onclick={closeModal}
      />
      <RoundButton
        customClass="w-18"
        label={isSubmitting ? t("common.saving") : t("common.save")}
        onclick={handleConfirm}
        disabled={isSubmitting || !canSave}
        loading={isSubmitting}
      />
    </div>
  </div>
</Modal>
