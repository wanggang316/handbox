<script lang="ts">
  import Tabs from '../ui/Tabs.svelte';
  import Button from '../ui/Button.svelte';
  import Input from '../ui/Input.svelte';
  import Select from '../ui/Select.svelte';
  import Toggle from '../ui/Toggle.svelte';

  let tab = 'providers';
  let showAdd = false;
  let provider = { name: '', type: 'openai', apiKey: '', baseUrl: '' };

  const providerTypes = [
    { value: 'openai', label: 'OpenAI' },
    { value: 'anthropic', label: 'Anthropic' },
    { value: 'google', label: 'Google AI' },
    { value: 'deepseek', label: 'DeepSeek' },
    { value: 'openrouter', label: 'OpenRouter' },
  ];
</script>

<div class="container">
  <div class="header">
    <h1>Settings</h1>
    <p>Configure your HandBox experience</p>
  </div>

  <Tabs {value} onChange={(v) => tab = v} items={[
    { value: 'providers', label: 'Providers' },
    { value: 'general', label: 'General' },
    { value: 'appearance', label: 'Appearance' },
    { value: 'account', label: 'Account' },
  ]} />

  {#if tab === 'providers'}
    <div class="section">
      <div class="section-header">
        <h2>AI Providers</h2>
        <Button on:click={() => showAdd = true}>Add Provider</Button>
      </div>

      <div class="list">
        {#each [1,2,3] as i}
          <div class="card">
            <div>
              <h3>OpenAI #{i}</h3>
              <p class="muted">openai</p>
              <p class="muted small">gpt-4o-mini</p>
            </div>
            <div class="actions">
              <Button variant="secondary">Use</Button>
              <Button variant="danger">Remove</Button>
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if tab === 'general'}
    <div class="section">
      <h2>General Settings</h2>
      <div class="grid">
        <Toggle label="Auto-scroll to latest message" />
        <Select label="Theme" options="{[{value:'system',label:'System'},{value:'light',label:'Light'},{value:'dark',label:'Dark'}]}" />
        <Select label="Theme color" options="{[{value:'system',label:'System'},{value:'blue',label:'Blue'},{value:'green',label:'Green'}]}" />
        <Select label="Language" options="{[{value:'zh-CN',label:'简体中文'},{value:'en-US',label:'English'}]}" />
      </div>
    </div>
  {/if}

  {#if tab === 'appearance'}
    <div class="section">
      <h2>Appearance</h2>
      <div class="grid">
        <Select label="Font size" options="{[{value:'small',label:'Small'},{value:'medium',label:'Medium'},{value:'large',label:'Large'}]}" />
      </div>
    </div>
  {/if}

  {#if tab === 'account'}
    <div class="section">
      <h2>Account</h2>
      <p class="muted">Coming soon…</p>
    </div>
  {/if}

  {#if showAdd}
    <div class="modal-overlay" role="dialog" aria-modal="true" onclick={(e) => { if (e.target === e.currentTarget) showAdd = false; }}>
      <div class="modal">
        <div class="modal-header">
          <h3>Add AI Provider</h3>
          <button class="close-btn" aria-label="Close" onclick={() => showAdd = false}>✕</button>
        </div>
        <div class="modal-content">
          <div class="form-grid">
            <Input label="Name" placeholder="e.g., OpenAI GPT-4" onInput={(v)=> provider.name = v} />
            <Select label="Type" options={providerTypes} onChange={(v)=> provider.type = v} />
            <Input label="API Key" placeholder="Your API key" onInput={(v)=> provider.apiKey = v} />
            <Input label="Base URL" placeholder="https://api.openai.com/v1" type="url" onInput={(v)=> provider.baseUrl = v} />
          </div>
          <div class="right">
            <Button variant="secondary" on:click={() => showAdd = false}>Cancel</Button>
            <Button>Add Provider</Button>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
.container { max-width: 960px; margin: 0 auto; padding: 2rem; }
.header { margin-bottom: 1.5rem; }
.header h1 { margin: 0 0 .5rem 0; font-size: 2rem; font-weight: 700; }
.muted { color: var(--text-secondary); }
.small { font-size: .85rem; opacity: .8; }
.section { margin: 1.5rem 0; }
.section-header { display:flex; align-items:center; justify-content:space-between; margin-bottom: 1rem; }
.list { display:flex; flex-direction:column; gap: .75rem; }
.card { display:flex; align-items:center; justify-content:space-between; padding: 1rem; border:1px solid var(--border-color); border-radius:8px; background: var(--bg-secondary); }
.actions { display:flex; gap: .5rem; }
.grid { display:grid; grid-template-columns: repeat(auto-fit,minmax(220px,1fr)); gap: 1rem; }
.form-grid { display:grid; grid-template-columns: 1fr; gap: 1rem; }
.right { display:flex; justify-content:flex-end; gap:.5rem; margin-top:1rem; }
.modal-overlay { position: fixed; inset:0; background: rgba(0,0,0,.5); display:flex; align-items:center; justify-content:center; }
.modal { background: var(--bg-primary); border-radius: 8px; box-shadow: 0 10px 25px rgba(0,0,0,.15); width: 560px; max-width: 95%; }
.modal-header { display:flex; justify-content:space-between; align-items:center; padding:1rem; border-bottom:1px solid var(--border-color); }
.modal-content { padding: 1rem; }
.close-btn { background:none; border:1px solid var(--border-color); border-radius:6px; padding:.25rem .5rem; cursor:pointer; color: var(--text-secondary); }
.close-btn:hover { background: var(--bg-hover); }
</style>


