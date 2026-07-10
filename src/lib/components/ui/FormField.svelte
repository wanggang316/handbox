<script lang="ts" module>
  import { getContext, setContext } from "svelte";

  const FORMFIELD_KEY = Symbol("formfield");

  // 由 FormField 提供、供内部表单控件（Input / Textarea / Select / Checkbox …）读取，
  // 从而共享同一 id（label 关联）、aria-describedby（error / hint）与 error 态。
  export interface FormFieldContext {
    readonly id: string;
    readonly describedby: string | undefined;
    readonly invalid: boolean;
  }

  // 控件在 FormField 内时返回 context，否则 undefined（独立使用，行为不变）。
  export function getFormField(): FormFieldContext | undefined {
    return getContext(FORMFIELD_KEY);
  }
</script>

<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    label?: string;
    required?: boolean;
    error?: string;
    hint?: string;
    // 不传则自动生成；用于把 label[for] 关联到内部控件。
    id?: string;
    children: Snippet;
  }

  let {
    label = "",
    required = false,
    error = "",
    hint = "",
    id,
    children,
  }: Props = $props();

  const autoId = `ff-${Math.random().toString(36).slice(2)}`;
  const fieldId = $derived(id ?? autoId);
  const errorId = $derived(`${fieldId}-error`);
  const hintId = $derived(`${fieldId}-hint`);

  // getter 让 describedby / invalid 随 error / hint 响应式更新。
  setContext<FormFieldContext>(FORMFIELD_KEY, {
    get id() {
      return fieldId;
    },
    get describedby() {
      return error ? errorId : hint ? hintId : undefined;
    },
    get invalid() {
      return !!error;
    },
  });
</script>

<div class="flex flex-col gap-1.5">
  {#if label}
    <label for={fieldId} class="text-sm font-medium text-base-content/80">
      {label}{#if required}<span class="ml-0.5 text-error">*</span>{/if}
    </label>
  {/if}

  {@render children()}

  {#if error}
    <p id={errorId} class="text-[0.8125rem] text-error">{error}</p>
  {:else if hint}
    <p id={hintId} class="text-xs text-base-content/60">{hint}</p>
  {/if}
</div>
