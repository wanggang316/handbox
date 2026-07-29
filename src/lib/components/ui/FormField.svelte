<script lang="ts" module>
  import { getContext, setContext } from "svelte";

  const FORMFIELD_KEY = Symbol("formfield");

  // Provided by FormField so inner controls (Input / Textarea / Select / Checkbox ...)
  // share one id (label association), aria-describedby (error / hint), and error state.
  export interface FormFieldContext {
    readonly id: string;
    readonly describedby: string | undefined;
    readonly invalid: boolean;
  }

  // Returns the context inside a FormField, undefined when the control is used standalone.
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
    // Auto-generated when omitted; links label[for] to the inner control.
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

  // Getters keep describedby / invalid reactive to error / hint changes.
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
