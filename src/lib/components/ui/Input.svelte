<script lang="ts">
  export let label = '';
  export let placeholder = '';
  export let type: 'text' | 'password' | 'url' | 'number' = 'text';
  export let value: string | number = '';
  export let onInput: (v: string) => void = () => {};
  let id = `inp-${Math.random().toString(36).slice(2)}`;
</script>

<label class="label" for={id}>{label}</label>
<input id={id} class="input" {type} bind:value on:input={(e) => onInput((e.currentTarget as HTMLInputElement).value)} placeholder={placeholder} />

<style>
.label { display:block; margin-bottom:.5rem; font-weight:500; }
.input { width:100%; padding:.5rem .75rem; border:1px solid var(--base-300); border-radius:6px; background:var(--base-100); color:var(--base-content); }
.input:focus{ outline:none; border-color: var(--primary); }
</style>


