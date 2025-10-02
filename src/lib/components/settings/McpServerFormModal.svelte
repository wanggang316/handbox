<script lang="ts">
  import Modal from "$lib/components/ui/Modal.svelte";
  import RoundButton from "$lib/components/ui/RoundButton.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import TextRow from "$lib/components/ui/table/TextRow.svelte";
  import TextareaRow from "$lib/components/ui/table/TextareaRow.svelte";
  import type {
    CreateMcpServerRequest,
    McpConnectionType,
    McpServer,
    UpdateMcpServerRequest,
  } from "$lib/types";
  interface EnvEntry {
    id: number;
    key: string;
    value: string;
  }

  interface HeaderEntry {
    id: number;
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
    }) => void;
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

  // 表单数据
  let formData = $state({
    name: "",
    displayName: "",
    description: "",
    connectionType: "stdio" as McpConnectionType,
    command: "",
    argsText: "",
    workingDir: "",
    endpoint: "",
    timeoutMs: "",
    enabled: true,
  });

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

  // 使用$effect替代$: 响应式语句
  $effect(() => {
    if (server) {
      // 编辑模式：填充表单数据
      formData.name = server.name;
      formData.displayName = server.displayName ?? "";
      formData.description = server.description ?? "";
      formData.connectionType = server.connectionType;
      formData.command = server.command;
      formData.argsText = server.args.join("\n");
      formData.workingDir = server.workingDir ?? "";
      formData.endpoint = server.endpoint ?? "";
      formData.timeoutMs = server.timeoutMs?.toString() ?? "";
      formData.enabled = server.enabled;

      envEntries = Object.entries(server.env).map(([key, value], index) => ({
        id: index,
        key,
        value,
      }));
      if (envEntries.length === 0) {
        envEntries = [{ id: Date.now(), key: "", value: "" }];
      }

      headerEntries = Object.entries(server.headers).map(([key, value], index) => ({
        id: index,
        key,
        value,
      }));
      if (headerEntries.length === 0) {
        headerEntries = [{ id: Date.now(), key: "", value: "" }];
      }
    } else {
      // 创建模式：重置表单数据
      formData = {
        name: "",
        displayName: "",
        description: "",
        connectionType: "stdio" as McpConnectionType,
        command: "",
        argsText: "",
        workingDir: "",
        endpoint: "",
        timeoutMs: "",
        enabled: true,
      };
      envEntries = [{ id: Date.now(), key: "", value: "" }];
      headerEntries = [{ id: Date.now(), key: "", value: "" }];
    }
    errors = {};
  });

  function closeModal() {
    modalRef?.handleClose();
  }

  function onModalClose() {
    onClose?.();
  }

  function addEnvEntry() {
    envEntries = [...envEntries, { id: Date.now(), key: "", value: "" }];
  }

  function removeEnvEntry(id: number) {
    if (envEntries.length === 1) {
      envEntries = [{ id: Date.now(), key: "", value: "" }];
      return;
    }
    envEntries = envEntries.filter((entry) => entry.id !== id);
  }

  function updateEnvEntry(id: number, field: "key" | "value", value: string) {
    envEntries = envEntries.map((entry) =>
      entry.id === id ? { ...entry, [field]: value } : entry,
    );
  }

  function addHeaderEntry() {
    headerEntries = [...headerEntries, { id: Date.now(), key: "", value: "" }];
  }

  function removeHeaderEntry(id: number) {
    if (headerEntries.length === 1) {
      headerEntries = [{ id: Date.now(), key: "", value: "" }];
      return;
    }
    headerEntries = headerEntries.filter((entry) => entry.id !== id);
  }

  function updateHeaderEntry(id: number, field: "key" | "value", value: string) {
    headerEntries = headerEntries.map((entry) =>
      entry.id === id ? { ...entry, [field]: value } : entry,
    );
  }

  function validate(): boolean {
    const nextErrors: Record<string, string> = {};

    if (!formData.name.trim()) {
      nextErrors.name = "请输入服务器名称";
    }

    if (formData.connectionType === 'stdio') {
      if (!formData.command.trim()) {
        nextErrors.command = "请输入执行命令";
      }
    } else {
      if (!formData.endpoint.trim()) {
        nextErrors.endpoint = "请输入端点URL";
      }
    }

    if (formData.timeoutMs && isNaN(Number(formData.timeoutMs))) {
      nextErrors.timeoutMs = "超时时间必须是数字";
    }

    errors = nextErrors;
    return Object.keys(nextErrors).length === 0;
  }

  function parseArgs(): string[] {
    return formData.argsText
      .split(/\r?\n|,/)
      .map((arg) => arg.trim())
      .filter(Boolean);
  }

  function parseEnv(): Record<string, string> {
    const result: Record<string, string> = {};
    for (const entry of envEntries) {
      const key = entry.key.trim();
      if (!key) continue;
      result[key] = entry.value;
    }
    return result;
  }

  function parseHeaders(): Record<string, string> {
    const result: Record<string, string> = {};
    for (const entry of headerEntries) {
      const key = entry.key.trim();
      if (!key) continue;
      result[key] = entry.value;
    }
    return result;
  }

  async function handleConfirm() {
    if (!validate()) return;

    isSubmitting = true;

    try {
      const base = {
        displayName: formData.displayName.trim() || undefined,
        description: formData.description.trim() || undefined,
        connectionType: formData.connectionType,
        ...(formData.connectionType === 'stdio' ? {
          command: formData.command.trim(),
          args: parseArgs(),
          workingDir: formData.workingDir.trim() || undefined,
          env: parseEnv(),
        } : {
          command: '', // 非 stdio 类型设为空字符串
          args: [],
          workingDir: undefined,
          env: {},
          endpoint: formData.endpoint.trim() || undefined,
          headers: parseHeaders(),
          timeoutMs: formData.timeoutMs ? Number(formData.timeoutMs) : undefined,
        }),
      };

      if (server) {
        const updatePayload: UpdateMcpServerRequest = {
          name: formData.name.trim(),
          ...base,
          enabled: formData.enabled,
        };
        onSave?.({ mode: "update", data: updatePayload });
      } else {
        const createPayload: CreateMcpServerRequest = {
          name: formData.name.trim(),
          ...base,
          enabled: formData.enabled,
        };
        onSave?.({ mode: "create", data: createPayload });
      }

      closeModal();
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
        {isEditMode ? "编辑 MCP 服务器" : "添加 MCP 服务器"}
      </h2>
      <div class="flex items-center gap-2">
        <Toggle bind:checked={formData.enabled} />
      </div>
    </div>

    <div class="flex-1 min-h-0 px-6 py-2 space-y-4 overflow-y-auto">
      <!-- 基本信息 -->
      <TableGroup>
        <TextRow
          label="名称"
          bind:value={formData.name}
          placeholder="唯一名称，如 filesystem"
        />
        <TextRow
          label="显示名称"
          bind:value={formData.displayName}
          placeholder="可选的用户可读名称"
        />

        <!-- 连接类型选择器 -->
        <div class="flex items-center justify-between px-4 py-3 border-b border-base-300 last:border-b-0">
          <span class="text-sm text-base-content/80">连接类型</span>
          <select
            class="px-3 py-2 text-sm bg-base-100 border border-base-300 rounded-lg focus:border-primary focus:outline-none"
            bind:value={formData.connectionType}
          >
            <option value="stdio">标准输入输出 (stdio)</option>
            <option value="sse">服务器发送事件 (SSE)</option>
            <option value="http">HTTP 端点</option>
          </select>
        </div>

        {#if formData.connectionType === 'stdio'}
          <TextRow
            label="命令"
            bind:value={formData.command}
            placeholder="如 npx 或 uvx"
          />
          <TextareaRow
            label="参数"
            bind:value={formData.argsText}
            placeholder="一行一个，或使用逗号分隔"
            rows={3}
          />
          <TextRow
            label="工作目录"
            bind:value={formData.workingDir}
            placeholder="可选"
            layout="vertical"
          />
        {:else}
          <TextRow
            label="端点URL"
            bind:value={formData.endpoint}
            placeholder="如 http://localhost:3000/mcp 或 ws://localhost:8080"
          />
          <TextRow
            label="超时时间 (毫秒)"
            bind:value={formData.timeoutMs}
            placeholder="可选，默认无超时"
          />
        {/if}
      </TableGroup>

      <!-- 环境变量 (只对 stdio 连接显示) -->
      {#if formData.connectionType === 'stdio'}
        <TableGroup>
          <div class="p-4 space-y-3">
            <div class="flex items-center justify-between">
              <span class="text-sm text-base-content/80">环境变量</span>
              <button
                class="text-primary text-sm hover:text-primary/80"
                type="button"
                onclick={addEnvEntry}
              >
                新增
              </button>
            </div>

            <div class="space-y-2">
              {#each envEntries as entry (entry.id)}
                <div class="grid grid-cols-[1fr_1fr_auto] gap-2 items-center">
                  <input
                    class="w-full px-3 py-2 text-sm bg-base-100 border border-base-300 rounded-lg focus:border-primary focus:outline-none"
                    placeholder="键"
                    value={entry.key}
                    oninput={(e) =>
                      updateEnvEntry(entry.id, "key", e.currentTarget.value)}
                  />
                  <input
                    class="w-full px-3 py-2 text-sm bg-base-100 border border-base-300 rounded-lg focus:border-primary focus:outline-none"
                    placeholder="值"
                    value={entry.value}
                    oninput={(e) =>
                      updateEnvEntry(entry.id, "value", e.currentTarget.value)}
                  />
                  <button
                    class="text-error text-sm hover:text-error/80 px-2"
                    type="button"
                    onclick={() => removeEnvEntry(entry.id)}
                  >
                    删除
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
              <span class="text-sm text-base-content/80">HTTP 头部</span>
              <button
                class="text-primary text-sm hover:text-primary/80"
                type="button"
                onclick={addHeaderEntry}
              >
                新增
              </button>
            </div>

            <div class="space-y-2">
              {#each headerEntries as entry (entry.id)}
                <div class="grid grid-cols-[1fr_1fr_auto] gap-2 items-center">
                  <input
                    class="w-full px-3 py-2 text-sm bg-base-100 border border-base-300 rounded-lg focus:border-primary focus:outline-none"
                    placeholder="头部名称"
                    value={entry.key}
                    oninput={(e) =>
                      updateHeaderEntry(entry.id, "key", e.currentTarget.value)}
                  />
                  <input
                    class="w-full px-3 py-2 text-sm bg-base-100 border border-base-300 rounded-lg focus:border-primary focus:outline-none"
                    placeholder="头部值"
                    value={entry.value}
                    oninput={(e) =>
                      updateHeaderEntry(entry.id, "value", e.currentTarget.value)}
                  />
                  <button
                    class="text-error text-sm hover:text-error/80 px-2"
                    type="button"
                    onclick={() => removeHeaderEntry(entry.id)}
                  >
                    删除
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
        label="取消"
        bgColor="bg-base-200"
        textColor="text-base-content/80"
        onclick={closeModal}
      />
      <RoundButton
        customClass="w-18"
        label={isSubmitting ? "保存中..." : "保存"}
        onclick={handleConfirm}
        disabled={isSubmitting || !canSave}
        loading={isSubmitting}
      />
    </div>
  </div>
</Modal>
