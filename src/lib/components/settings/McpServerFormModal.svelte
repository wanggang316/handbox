<script lang="ts">
  import FormModal from "$lib/components/ui/FormModal.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import { Trash2 } from "@lucide/svelte";
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

  const CONNECTION_OPTIONS = [
    { value: "stdio", labelKey: "provider.connectionStdio" },
    { value: "sse", labelKey: "provider.connectionSse" },
    { value: "http", labelKey: "provider.connectionHttp" },
  ] as const;

  let formData = $state<FormState>({ ...EMPTY_FORM });

  const isEditMode = $derived(server !== null);

  const canSave = $derived.by(() => {
    const hasName = formData.name.trim();
    const hasValidConnection = formData.connectionType === 'stdio'
      ? formData.command.trim()
      : formData.endpoint.trim();
    return Boolean(hasName && hasValidConnection) && !isSubmitting;
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

  $effect(() => {
    if (open) {
      initialiseForm(server);
    }
  });

  // Setting open = false triggers the Modal close animation.
  function closeModal() {
    open = false;
  }

  function onModalClose() {
    onClose?.();
  }

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

    if (!formData.name.trim()) {
      nextErrors.name = t("provider.validateMcpName");
    }

    if (formData.connectionType === 'stdio') {
      if (!formData.command.trim()) {
        nextErrors.command = t("provider.validateCommand");
      }
    } else {
      if (!formData.endpoint.trim()) {
        nextErrors.endpoint = t("provider.validateEndpoint");
      }
      if (formData.timeoutMs && isNaN(Number(formData.timeoutMs))) {
        nextErrors.timeoutMs = t("provider.validateTimeout");
      }
    }

    errors = nextErrors;
    return Object.keys(nextErrors).length === 0;
  }

  // Args may be separated by newlines or commas.
  function parseArgs(): string[] {
    return formData.argsText
      .split(/\r?\n|,/)
      .map(arg => arg.trim())
      .filter(Boolean);
  }

  function parseEnv(): Record<string, string> {
    return envEntries.reduce<Record<string, string>>((acc, entry) => {
      const key = entry.key.trim();
      if (key) acc[key] = entry.value;
      return acc;
    }, {});
  }

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

<FormModal
  bind:open={open}
  size="md"
  title={isEditMode ? t("provider.editMcpTitle") : t("provider.addMcpTitle")}
  onClose={onModalClose}
  saving={isSubmitting}
  submitLabel={isSubmitting ? t("common.saving") : t("common.save")}
  submitDisabled={isSubmitting || !canSave}
  onSubmit={handleConfirm}
>
  <div class="flex flex-col gap-1">
    <input
      class="modal-title-input"
      bind:value={formData.name}
      placeholder={t("provider.mcpNamePlaceholder")}
      aria-invalid={!!errors.name}
    />
    {#if errors.name}
      <span class="text-xs text-error">{errors.name}</span>
    {/if}
    <input
      class="w-full bg-transparent text-sm text-base-content/80 outline-none placeholder:text-base-content/35"
      bind:value={formData.displayName}
      placeholder={t("provider.mcpDisplayNamePlaceholder")}
    />
  </div>

  <div class="mt-4 flex items-center justify-between">
    <span class="text-sm text-base-content/80">{t("common.enabled")}</span>
    <Toggle bind:checked={formData.enabled} />
  </div>

  <div class="mt-5 flex rounded-lg border border-[var(--hairline)] p-0.5">
    {#each CONNECTION_OPTIONS as option (option.value)}
      <button
        type="button"
        class="flex-1 cursor-pointer rounded-md py-1.5 text-center text-sm {formData.connectionType ===
        option.value
          ? 'bg-base-300 text-base-content'
          : 'text-base-content/60 hover:text-base-content'}"
        onclick={() => (formData.connectionType = option.value)}
      >
        {t(option.labelKey)}
      </button>
    {/each}
  </div>

  {#if formData.connectionType === 'stdio'}
    <div class="mt-5 flex flex-col gap-4">
      <label class="flex flex-col gap-1">
        <span class="text-xs text-base-content/70">{t("provider.mcpCommand")}</span>
        <input
          class="field w-full px-2.5 py-1.5 text-sm"
          class:is-error={!!errors.command}
          bind:value={formData.command}
          placeholder={t("provider.mcpCommandPlaceholder")}
          aria-invalid={!!errors.command}
        />
        {#if errors.command}
          <span class="text-xs text-error">{errors.command}</span>
        {/if}
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-xs text-base-content/70">{t("provider.mcpArgs")}</span>
        <textarea
          class="field w-full px-2.5 py-1.5 text-sm"
          rows={3}
          bind:value={formData.argsText}
          placeholder={t("provider.mcpArgsPlaceholder")}
        ></textarea>
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-xs text-base-content/70">{t("provider.mcpWorkingDir")}</span>
        <input
          class="field w-full px-2.5 py-1.5 text-sm"
          bind:value={formData.workingDir}
          placeholder={t("provider.optional")}
        />
      </label>

      <div class="flex flex-col gap-2">
        <span class="form-section-label">{t("provider.envVars")}</span>
        {#each envEntries as entry, index (index)}
          <div class="grid grid-cols-[1fr_1fr_auto] items-center gap-2">
            <input
              class="field px-2.5 py-1.5 text-sm"
              placeholder={t("provider.envKeyPlaceholder")}
              value={entry.key}
              oninput={(e) => updateEnvEntry(index, "key", e.currentTarget.value)}
            />
            <input
              class="field px-2.5 py-1.5 text-sm"
              placeholder={t("provider.envValuePlaceholder")}
              value={entry.value}
              oninput={(e) => updateEnvEntry(index, "value", e.currentTarget.value)}
            />
            <button
              type="button"
              class="cursor-pointer p-1 text-base-content/40 transition-colors hover:text-error"
              title={t("common.delete")}
              aria-label={t("common.delete")}
              onclick={() => removeEnvEntry(index)}
            >
              <Trash2 size={14} />
            </button>
          </div>
        {/each}
        <button
          type="button"
          class="w-full cursor-pointer rounded-md border border-dashed border-[var(--hairline)] py-1.5 text-sm text-base-content/60 hover:border-[var(--hairline-strong)] hover:text-base-content"
          onclick={addEnvEntry}
        >
          + {t("provider.addEntry")}
        </button>
      </div>
    </div>
  {:else}
    <div class="mt-5 flex flex-col gap-4">
      <label class="flex flex-col gap-1">
        <span class="text-xs text-base-content/70">{t("provider.mcpEndpoint")}</span>
        <input
          class="field w-full px-2.5 py-1.5 text-sm"
          class:is-error={!!errors.endpoint}
          bind:value={formData.endpoint}
          placeholder={t("provider.mcpEndpointPlaceholder")}
          aria-invalid={!!errors.endpoint}
        />
        {#if errors.endpoint}
          <span class="text-xs text-error">{errors.endpoint}</span>
        {/if}
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-xs text-base-content/70">{t("provider.mcpTimeout")}</span>
        <input
          class="field w-full px-2.5 py-1.5 text-sm"
          class:is-error={!!errors.timeoutMs}
          bind:value={formData.timeoutMs}
          placeholder={t("provider.mcpTimeoutPlaceholder")}
          aria-invalid={!!errors.timeoutMs}
        />
        {#if errors.timeoutMs}
          <span class="text-xs text-error">{errors.timeoutMs}</span>
        {/if}
      </label>

      <div class="flex flex-col gap-2">
        <span class="form-section-label">{t("provider.httpHeaders")}</span>
        {#each headerEntries as entry, index (index)}
          <div class="grid grid-cols-[1fr_1fr_auto] items-center gap-2">
            <input
              class="field px-2.5 py-1.5 text-sm"
              placeholder={t("provider.headerKeyPlaceholder")}
              value={entry.key}
              oninput={(e) => updateHeaderEntry(index, "key", e.currentTarget.value)}
            />
            <input
              class="field px-2.5 py-1.5 text-sm"
              placeholder={t("provider.headerValuePlaceholder")}
              value={entry.value}
              oninput={(e) => updateHeaderEntry(index, "value", e.currentTarget.value)}
            />
            <button
              type="button"
              class="cursor-pointer p-1 text-base-content/40 transition-colors hover:text-error"
              title={t("common.delete")}
              aria-label={t("common.delete")}
              onclick={() => removeHeaderEntry(index)}
            >
              <Trash2 size={14} />
            </button>
          </div>
        {/each}
        <button
          type="button"
          class="w-full cursor-pointer rounded-md border border-dashed border-[var(--hairline)] py-1.5 text-sm text-base-content/60 hover:border-[var(--hairline-strong)] hover:text-base-content"
          onclick={addHeaderEntry}
        >
          + {t("provider.addEntry")}
        </button>
      </div>
    </div>
  {/if}
</FormModal>
