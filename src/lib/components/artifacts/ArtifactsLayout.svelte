<script lang="ts">
  import Button from '../ui/Button.svelte';
  let artifacts = Array.from({ length: 12 }).map((_, i) => ({ id: `${i}`, name: `Artifact ${i+1}`, description: 'Short description', createdAt: Date.now()-i*123456 }));
  let search = '';
  let selected: any = null;
</script>

<div class="layout">
  <aside class="sidebar">
    <div class="sidebar-header">
      <h2>Artifacts</h2>
      <Button size="sm">Create</Button>
    </div>
    <div class="search">
      <input placeholder="Search artifacts..." bind:value={search} />
      <svg class="icon" width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"/></svg>
    </div>
    <div class="list">
      {#each artifacts.filter(a => a.name.toLowerCase().includes(search.toLowerCase())) as a (a.id)}
        <div
          class="item"
          class:selected={selected?.id===a.id}
          role="button"
          tabindex="0"
          onclick={() => selected = a}
          onkeydown={(event) => {
            if (event.key === 'Enter' || event.key === ' ') {
              event.preventDefault();
              selected = a;
            }
          }}
        >
          <div class="avatar">A</div>
          <div class="info">
            <h4>{a.name}</h4>
            <p class="muted">{new Date(a.createdAt).toLocaleDateString()}</p>
          </div>
          <button class="delete" aria-label="Delete">🗑</button>
        </div>
      {/each}
    </div>
  </aside>

  <main class="main">
    {#if selected}
      <div class="viewer">
        <div class="viewer-header">
          <div class="title">
            <div class="avatar">A</div>
            <div>
              <h1>{selected.name}</h1>
              <div class="meta"><span class="badge">artifact</span><span>Created {new Date(selected.createdAt).toLocaleDateString()}</span></div>
            </div>
          </div>
          <div class="actions">
            <Button variant="ghost">Edit</Button>
            <Button variant="ghost">Copy</Button>
            <Button variant="ghost">Download</Button>
          </div>
        </div>
        <div class="content"><div class="text">{selected.description}</div></div>
      </div>
    {:else}
      <div class="empty">
        <div class="icon">📦</div>
        <h3>No artifact selected</h3>
        <p class="muted">Select an artifact from the sidebar to view its content</p>
      </div>
    {/if}
  </main>
</div>

<style>
.layout { display:flex; height:100vh; }
.sidebar { width:320px; background:var(--base-200); border-right:1px solid var(--base-300); display:flex; flex-direction:column; }
.sidebar-header { padding:1rem; border-bottom:1px solid var(--base-300); display:flex; justify-content:space-between; align-items:center; }
.search { position:relative; padding:1rem; }
.search input { width:100%; padding:.5rem .75rem .5rem 2.25rem; border:1px solid var(--base-300); border-radius:6px; background:var(--base-100); color:var(--base-content); }
.search .icon { position:absolute; left:1.75rem; top:50%; transform: translateY(-50%); color: color-mix(in oklch, var(--base-content) 80%, transparent); }
.list { flex:1; overflow:auto; padding:.5rem; }
.item { display:flex; align-items:center; gap:.75rem; padding:.75rem; border-radius:6px; cursor:pointer; position:relative; }
.item:hover { background: var(--base-300); }
.item.selected { background: var(--primary); color: var(--base-100); }
.avatar { width:32px; height:32px; background:var(--primary); color:var(--primary-content); display:flex; align-items:center; justify-content:center; border-radius:6px; font-weight:700; }
.info h4 { margin:0 0 .25rem 0; font-size:.9rem; font-weight:600; }
.muted { color: color-mix(in oklch, var(--base-content) 80%, transparent); }
.delete { position:absolute; right:.5rem; background:none; border:none; color:color-mix(in oklch, var(--base-content) 80%, transparent); opacity:0; cursor:pointer; border-radius:4px; padding:.25rem; }
.item:hover .delete { opacity:1; }
.main { flex:1; display:flex; flex-direction:column; overflow:hidden; }
.viewer { flex:1; display:flex; flex-direction:column; }
.viewer-header { padding:1.5rem; border-bottom:1px solid var(--base-300); display:flex; justify-content:space-between; align-items:center; }
.title { display:flex; align-items:center; gap:1rem; }
.meta { display:flex; gap:.75rem; align-items:center; color: color-mix(in oklch, var(--base-content) 80%, transparent); margin-top:.25rem; }
.badge { background: var(--base-200); color: color-mix(in oklch, var(--base-content) 80%, transparent); padding:.125rem .5rem; border-radius:4px; font-size:.75rem; }
.content { flex:1; overflow:auto; padding:1.5rem; }
.empty { flex:1; display:flex; align-items:center; justify-content:center; flex-direction:column; gap:.5rem; color: color-mix(in oklch, var(--base-content) 80%, transparent); }
</style>
