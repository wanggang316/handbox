<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import Modal from '$lib/components/ui/Modal.svelte';
  import Input from '$lib/components/ui/Input.svelte';
  import Textarea from '$lib/components/ui/Textarea.svelte';
  import RoundButton from '$lib/components/ui/RoundButton.svelte';
  import Toggle from '$lib/components/ui/Toggle.svelte';
  import type {
    CreateMcpServerRequest,
    McpServer,
    UpdateMcpServerRequest
  } from '$lib/types';

  interface EnvEntry {
    id: number;
    key: string;
    value: string;
  }

  interface Props {
    open?: boolean;
    server?: McpServer | null;
  }

  let {
    open = $bindable(false),
    server = $bindable<McpServer | null>(null)
  }: Props = $props();

  const dispatch = createEventDispatcher<{
    close: void;
    save: { mode: 'create' | 'update'; data: CreateMcpServerRequest | UpdateMcpServerRequest };
  }>();

  let modalRef: Modal;
  let isSubmitting = $state(false);
  let errors = $state<Record<string, string>>({});
  let envEntries = $state<EnvEntry[]>([]);

  let name = $state('');
  let displayName = $state('');
  let description = $state('');
  let command = $state('');
  let argsText = $state('');
  let workingDir = $state('');
  let enabled = $state(true);

  $effect(() => {
    if (server) {
      name = server.name;
      displayName = server.displayName ?? '';
      description = server.description ?? '';
      command = server.command;
      argsText = server.args.join('\n');
      workingDir = server.workingDir ?? '';
      enabled = server.enabled;
      envEntries = Object.entries(server.env).map(([key, value], index) => ({
        id: index,
        key,
        value
      }));
      if (envEntries.length === 0) {
        envEntries = [{ id: Date.now(), key: '', value: '' }];
      }
    } else {
      name = '';
      displayName = '';
      description = '';
      command = '';
      argsText = '';
      workingDir = '';
      enabled = true;
      envEntries = [{ id: Date.now(), key: '', value: '' }];
    }
    errors = {};
  });

  function closeModal() {
    modalRef?.handleClose();
  }

  function handleClose() {
    dispatch('close');
  }

  function addEnvEntry() {
    envEntries = [...envEntries, { id: Date.now(), key: '', value: '' }];
  }

  function removeEnvEntry(id: number) {
    if (envEntries.length === 1) {
      envEntries = [{ id: Date.now(), key: '', value: '' }];
      return;
    }
    envEntries = envEntries.filter(entry => entry.id !== id);
  }

  function updateEnvEntry(id: number, updates: Partial<EnvEntry>) {
    envEntries = envEntries.map(entry =>
      entry.id === id ? { ...entry, ...updates } : entry
    );
  }

  function validate(): boolean {
    const nextErrors: Record<string, string> = {};

    if (!name.trim()) {
      nextErrors.name = '请输入服务器名称';
    }
    if (!command.trim()) {
      nextErrors.command = '请输入执行命令';
    }

    errors = nextErrors;
    return Object.keys(nextErrors).length === 0;
  }

  function parseArgs(): string[] {
    return argsText
      .split(/\r?\n|,/)
      .map(arg => arg.trim())
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

  async function handleSave() {
    if (!validate()) return;

    isSubmitting = true;

    try {
      const base = {
        displayName: displayName.trim() || undefined,
        description: description.trim() || undefined,
        command: command.trim(),
        args: parseArgs(),
        workingDir: workingDir.trim() || undefined,
        env: parseEnv()
      };

      if (server) {
        const updatePayload: UpdateMcpServerRequest = {
          name: name.trim(),
          ...base,
          enabled
        };
        dispatch('save', { mode: 'update', data: updatePayload });
      } else {
        const createPayload: CreateMcpServerRequest = {
          name: name.trim(),
          ...base,
          enabled
        };
        dispatch('save', { mode: 'create', data: createPayload });
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
  title={server ? '编辑 MCP 服务器' : '新增 MCP 服务器'}
  showCloseButton={false}
  onClose={handleClose}
>
  <div class="flex flex-col gap-4 px-6 py-4">
    <div class="grid grid-cols-2 gap-4">
      <div class="flex flex-col gap-1">
        <Input
          label="名称"
          placeholder="唯一名称，例如 filesystem"
          bind:value={name}
        />
        {#if errors.name}
          <p class="text-xs text-error">{errors.name}</p>
        {/if}
      </div>
      <Input
        label="显示名称"
        placeholder="可选的用户可读名称"
        bind:value={displayName}
      />
    </div>

    <div class="flex flex-col gap-1">
      <span class="text-sm font-medium text-base-content/80">描述</span>
      <Textarea
        placeholder="用于说明服务器能力"
        rows={3}
        bind:value={description}
      />
    </div>

    <div class="grid grid-cols-2 gap-4">
      <div class="flex flex-col gap-1">
        <Input
          label="命令"
          placeholder="可执行文件或脚本"
          bind:value={command}
        />
        {#if errors.command}
          <p class="text-xs text-error">{errors.command}</p>
        {/if}
      </div>
      <Input
        label="工作目录"
        placeholder="可选"
        bind:value={workingDir}
      />
    </div>

    <div class="flex flex-col gap-1">
      <span class="text-sm font-medium text-base-content/80">参数</span>
      <Textarea
        placeholder="一行一个，或使用逗号分隔"
        rows={3}
        bind:value={argsText}
      />
    </div>

    <div class="flex flex-col gap-2">
      <div class="flex items-center justify-between">
        <span class="text-sm text-base-content/80">环境变量</span>
        <button class="text-primary text-sm" type="button" onclick={addEnvEntry}>新增</button>
      </div>

      <div class="flex flex-col gap-2">
        {#each envEntries as entry (entry.id)}
          <div class="grid grid-cols-[1fr_1fr_auto] gap-2 items-center">
            <Input
              placeholder="键"
              value={entry.key}
              onInput={(value) => updateEnvEntry(entry.id, { key: value })}
            />
            <Input
              placeholder="值"
              value={entry.value}
              onInput={(value) => updateEnvEntry(entry.id, { value })}
            />
            <button
              class="text-error text-sm"
              type="button"
              onclick={() => removeEnvEntry(entry.id)}
            >删除</button>
          </div>
        {/each}
      </div>
    </div>

    <div class="flex items-center justify-between border-t border-base-300 pt-4 mt-2">
      <div>
        <label class="flex items-center gap-2 text-sm text-base-content/80">
          <Toggle bind:checked={enabled} />
          启用服务器
        </label>
      </div>

      <div class="flex gap-3">
        <RoundButton
          customClass="w-18"
          label="取消"
          bgColor="bg-base-200"
          textColor="text-base-content/80"
          hoverColor="hover:text-base-content"
          onclick={closeModal}
        />
        <RoundButton
          customClass="w-24"
          label={isSubmitting ? '保存中...' : '保存'}
          onclick={handleSave}
          disabled={isSubmitting}
        />
      </div>
    </div>
  </div>
</Modal>
