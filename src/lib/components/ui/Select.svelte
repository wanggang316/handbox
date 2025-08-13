<script lang="ts">
  export let label = '';
  export let value: string | number | undefined;
  export let options: Array<{ value: string; label: string }>= [];
  export let onChange: (v: string) => void = () => {};
  let id = `sel-${Math.random().toString(36).slice(2)}`;
</script>

<label class="label" for={id}>{label}</label>
<select id={id} class="select" bind:value on:change={(e) => onChange((e.currentTarget as HTMLSelectElement).value)}>
  {#each options as opt (opt.value)}
    <option value={opt.value}>{opt.label}</option>
  {/each}
  <slot />
  </select>

<style>
.label { display:block; margin-bottom:.5rem; font-weight:500; }
.select { width:100%; padding:.5rem; border:1px solid var(--border-color); border-radius:6px; background:var(--bg-primary); color:var(--text-primary); }
</style>


