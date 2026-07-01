<script lang="ts">
  interface Props {
    label?: string;
    placeholder?: string;
    type?: 'text' | 'password' | 'url' | 'number';
    value?: string | number;
    onInput?: (v: string) => void;
    disabled?: boolean;
    required?: boolean;
    error?: string;
  }

  let {
    label = '',
    placeholder = '',
    type = 'text',
    value = $bindable(''),
    onInput = () => {},
    disabled = false,
    required = false,
    error = '',
  }: Props = $props();

  const id = `inp-${Math.random().toString(36).slice(2)}`;
  const errorId = `${id}-error`;
</script>

<label class="label" for={id}>
  {label}{#if required}<span class="required-marker" aria-hidden="true">*</span>{/if}
</label>
<input
  {id}
  class="field w-full px-3 py-2 text-sm"
  class:is-error={!!error}
  {type}
  {placeholder}
  {disabled}
  {required}
  aria-required={required ? 'true' : undefined}
  aria-invalid={error ? 'true' : undefined}
  aria-describedby={error ? errorId : undefined}
  bind:value
  oninput={(e) => onInput((e.currentTarget as HTMLInputElement).value)}
/>
{#if error}
  <p id={errorId} class="error-message">{error}</p>
{/if}

<style>
.label { display:block; margin-bottom:.5rem; font-weight:500; font-size:.875rem; }
.required-marker { color: var(--field-error); margin-left:.125rem; }
.error-message { margin-top:.375rem; color: var(--field-error); font-size:.8125rem; }
</style>
