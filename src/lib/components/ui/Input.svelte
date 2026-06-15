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
  class="input"
  class:has-error={!!error}
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
.label { display:block; margin-bottom:.5rem; font-weight:500; }
.required-marker { color: var(--error); margin-left:.125rem; }
.input { width:100%; padding:.5rem .75rem; border:1px solid var(--base-300); border-radius:6px; background:var(--base-100); color:var(--base-content); }
.input:focus{ border-color: var(--primary); }
.input:disabled{ opacity:.5; cursor:not-allowed; }
.input.has-error{ border-color: var(--error); }
.error-message { margin-top:.375rem; color: var(--error); font-size:.8125rem; }
</style>
