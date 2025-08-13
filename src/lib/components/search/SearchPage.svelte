<script lang="ts">
  let q = '';
  const filters = [
    { value: 'all', label: 'All' },
    { value: 'messages', label: 'Messages' },
    { value: 'sessions', label: 'Sessions' },
    { value: 'artifacts', label: 'Artifacts' }
  ];
  let f = 'all';
  let results = Array.from({ length: 6 }).map((_, i) => ({ id: `${i}`, type: i % 3 === 0 ? 'message' : (i % 3 === 1 ? 'session' : 'artifact'), title: `Result ${i+1}`, content: 'Lorem ipsum dolor sit amet', date: Date.now() - i * 1000000 }));
</script>

<div class="container">
  <div class="header">
    <h1>Search</h1>
    <p>Search through your conversations, artifacts, and more</p>
  </div>

  <div class="search">
    <div class="bar">
      <svg width="20" height="20" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" /></svg>
      <input placeholder="Search conversations, artifacts, and more..." bind:value={q} />
      {#if q}
        <button class="clear" aria-label="Clear" onclick={() => q = ''}>✕</button>
      {/if}
    </div>
    <div class="filters">
      {#each filters as it (it.value)}
        <button class="filter" class:active={f === it.value} onclick={() => f = it.value}>{it.label}</button>
      {/each}
    </div>
  </div>

  <div class="results">
    {#each results as r (r.id)}
      <div class="item">
        <div class="icon"><svg width="16" height="16" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" /></svg></div>
        <div class="content">
          <div class="header"><h4>{r.title}</h4><span class="type">{r.type}</span></div>
          <p class="snippet">{r.content}</p>
          <div class="meta"><span>{new Date(r.date).toLocaleDateString()}</span></div>
        </div>
      </div>
    {/each}
  </div>
</div>

<style>
.container { max-width: 800px; margin: 0 auto; padding: 2rem; }
.header { margin-bottom: 1.5rem; }
.header h1 { margin: 0 0 .5rem 0; font-size: 2rem; font-weight: 700; }
.search .bar { position: relative; margin-bottom: 1rem; }
.search .bar svg { position: absolute; left: .75rem; top: 50%; transform: translateY(-50%); color: var(--text-secondary); }
.search input { width: 100%; padding: .75rem 2.5rem; border: 2px solid var(--border-color); border-radius: 12px; background: var(--bg-primary); color: var(--text-primary); }
.search input:focus { outline:none; border-color: var(--bg-accent); }
.clear { position: absolute; right: .5rem; top: 50%; transform: translateY(-50%); border: none; background: none; color: var(--text-secondary); cursor:pointer; border-radius: 6px; padding: .25rem; }
.clear:hover { background: var(--bg-hover); color: var(--text-primary); }
.filters { display:flex; gap:.5rem; flex-wrap:wrap; }
.filter { padding: .5rem 1rem; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 20px; cursor: pointer; color: var(--text-secondary); }
.filter.active { background: var(--bg-accent); color: var(--text-accent); border-color: var(--bg-accent); }
.results { display:flex; flex-direction:column; gap:1rem; margin-top:1rem; }
.item { display:flex; gap:1rem; padding:1rem; background:var(--bg-secondary); border:1px solid var(--border-color); border-radius:8px; }
.icon { width:32px; height:32px; background: var(--bg-accent); color: var(--text-accent); border-radius:6px; display:flex; align-items:center; justify-content:center; flex-shrink:0; }
.header { display:flex; align-items:center; gap:.5rem; }
.header h4 { margin:0; font-size:1rem; font-weight:600; }
.snippet { margin:.25rem 0; color: var(--text-secondary); }
.type { background: var(--bg-primary); color: var(--text-secondary); padding: .125rem .5rem; border-radius: 4px; font-size: .75rem; text-transform: capitalize; }
</style>


