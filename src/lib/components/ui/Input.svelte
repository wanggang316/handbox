<script lang="ts">
  import { getFormField } from "./FormField.svelte";

  interface Props {
    label?: string;
    placeholder?: string;
    type?: "text" | "password" | "url" | "number";
    value?: string | number;
    onInput?: (v: string) => void;
    disabled?: boolean;
    required?: boolean;
    error?: string;
    /**
     * Identifier-style input: turns off the platform's autocapitalize /
     * autocorrect / spellcheck so the value is stored exactly as typed. Needed
     * for anything matched literally — a tool name, a path, an argument key —
     * where macOS would otherwise capitalize the first letter and silently
     * break the match.
     */
    literal?: boolean;
  }

  let {
    label = "",
    placeholder = "",
    type = "text",
    value = $bindable(""),
    onInput = () => {},
    disabled = false,
    required = false,
    error = "",
    literal = false,
  }: Props = $props();

  // Inside a FormField the container supplies id / aria / error state, so the
  // control skips rendering its own label / error.
  const ff = getFormField();
  const autoId = `inp-${Math.random().toString(36).slice(2)}`;
  const ownErrorId = `${autoId}-error`;

  const controlId = $derived(ff ? ff.id : autoId);
  const invalid = $derived(ff ? ff.invalid : !!error);
  const describedby = $derived(
    ff ? ff.describedby : error ? ownErrorId : undefined,
  );
  const showOwnLabel = $derived(!ff && !!label);
  const showOwnError = $derived(!ff && !!error);
</script>

{#if showOwnLabel}
  <label class="label" for={controlId}>
    {label}{#if required}<span class="required-marker" aria-hidden="true">*</span
      >{/if}
  </label>
{/if}
<input
  id={controlId}
  class="field w-full px-3 py-2 text-sm"
  class:is-error={invalid}
  {type}
  {placeholder}
  {disabled}
  {required}
  aria-required={required ? "true" : undefined}
  aria-invalid={invalid ? "true" : undefined}
  aria-describedby={describedby}
  autocapitalize={literal ? "off" : undefined}
  autocorrect={literal ? "off" : undefined}
  spellcheck={literal ? false : undefined}
  bind:value
  oninput={(e) => onInput((e.currentTarget as HTMLInputElement).value)}
/>
{#if showOwnError}
  <p id={ownErrorId} class="error-message">{error}</p>
{/if}

<style>
  .label {
    display: block;
    margin-bottom: 0.5rem;
    font-weight: 500;
    font-size: 0.875rem;
  }
  .required-marker {
    color: var(--field-error);
    margin-left: 0.125rem;
  }
  .error-message {
    margin-top: 0.375rem;
    color: var(--field-error);
    font-size: 0.8125rem;
  }
</style>
