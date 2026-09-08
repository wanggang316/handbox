<script lang="ts">
  /**
   * Editor for a custom tool's parameter list.
   *
   * A parameter is two things at once: a field in the schema the model fills in,
   * and the state path the tool's view binds to. The binding path is shown on
   * every row for exactly that reason — it is what the author copies into the
   * GenUI spec, and there is nowhere else to read it.
   *
   * The types offered are the ones the GenUI catalog can consume; see
   * `types/toolDefinition.ts`.
   */
  import { Plus, Trash2 } from "@lucide/svelte";
  import Input from "$lib/components/ui/Input.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import Checkbox from "$lib/components/ui/Checkbox.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import { paramBindingPath } from "$lib/components/genui/bindings";
  import { t } from "$lib/i18n";
  import type { ToolParam, ToolParamType } from "$lib/types/toolDefinition";

  interface Props {
    parameters: ToolParam[];
    onChange: (parameters: ToolParam[]) => void;
    readOnly?: boolean;
  }

  let { parameters, onChange, readOnly = false }: Props = $props();

  const typeOptions = $derived([
    { value: "string", label: t("settings.tools.paramType.string") },
    { value: "number", label: t("settings.tools.paramType.number") },
    { value: "boolean", label: t("settings.tools.paramType.boolean") },
    { value: "stringList", label: t("settings.tools.paramType.stringList") },
    { value: "stringMatrix", label: t("settings.tools.paramType.stringMatrix") },
    { value: "keyValueList", label: t("settings.tools.paramType.keyValueList") },
  ]);

  function patch(index: number, changes: Partial<ToolParam>): void {
    onChange(
      parameters.map((param, i) =>
        i === index ? { ...param, ...changes } : param,
      ),
    );
  }

  function add(): void {
    onChange([
      ...parameters,
      { name: "", type: "string", description: "", required: true },
    ]);
  }

  function remove(index: number): void {
    onChange(parameters.filter((_, i) => i !== index));
  }
</script>

<div class="flex flex-col gap-3">
  {#if parameters.length === 0}
    <p class="text-[13px] text-base-content/55">
      {t("settings.tools.params.empty")}
    </p>
  {/if}

  {#each parameters as param, index (index)}
    <div
      class="rounded-lg border border-[var(--hairline)] bg-base-200/40 p-3 flex flex-col gap-2"
    >
      <div class="flex items-start gap-2">
        <div class="min-w-0 flex-1">
          <Input
            value={param.name}
            placeholder={t("settings.tools.params.namePlaceholder")}
            literal
            disabled={readOnly}
            onInput={(value) => patch(index, { name: value })}
          />
        </div>
        <div class="w-40 shrink-0">
          <Select
            value={param.type}
            options={typeOptions}
            disabled={readOnly}
            onChange={(value) => patch(index, { type: value as ToolParamType })}
          />
        </div>
        {#if !readOnly}
          <Button
            variant="clear"
            size="icon-sm"
            class="mt-1 text-base-content/40 enabled:hover:text-error"
            ariaLabel={t("settings.tools.params.remove")}
            onclick={() => remove(index)}
          >
            <Trash2 size={16} />
          </Button>
        {/if}
      </div>

      <Input
        value={param.description}
        placeholder={t("settings.tools.params.descriptionPlaceholder")}
        disabled={readOnly}
        onInput={(value) => patch(index, { description: value })}
      />

      <div class="flex items-center justify-between gap-3">
        <Checkbox
          checked={param.required}
          disabled={readOnly}
          onCheckedChange={(checked) => patch(index, { required: checked })}
        >
          {t("settings.tools.params.required")}
        </Checkbox>
        <!-- The path this parameter is bound by, ready to paste into the view's
             spec. Only meaningful once the parameter has a name. -->
        {#if param.name}
          <code class="text-[11px] text-base-content/50">
            {paramBindingPath(param.name)}
          </code>
        {/if}
      </div>
    </div>
  {/each}

  {#if !readOnly}
    <Button variant="secondary" size="sm" class="w-fit" onclick={add}>
      <Plus size={14} />
      {t("settings.tools.params.add")}
    </Button>
  {/if}
</div>
