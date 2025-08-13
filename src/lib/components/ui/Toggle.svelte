<script lang="ts">
  export let label = '';
  export let checked = false;
  export let onChange: (v: boolean) => void = () => {};
  let id = `tgl-${Math.random().toString(36).slice(2)}`;
</script>

<label class="toggle">
  <input id={id} type="checkbox" bind:checked on:change={(e) => onChange((e.currentTarget as HTMLInputElement).checked)} />
  <span class="slider" aria-hidden="true"></span>
  <span class="text">{label}</span>
</label>

<style>
.toggle { display:flex; align-items:center; gap:.5rem; cursor:pointer; }
.toggle input { display:none; }
.slider { width:36px; height:20px; border-radius:999px; background:var(--border-color); position:relative; transition: all .2s; }
.slider::after { content:''; position:absolute; top:2px; left:2px; width:16px; height:16px; border-radius:50%; background:#fff; transition: transform .2s; box-shadow:0 1px 2px rgba(0,0,0,.2) }
input:checked + .slider { background: var(--bg-accent); }
input:checked + .slider::after { transform: translateX(16px); }
.text { color: var(--text-primary); }
</style>


