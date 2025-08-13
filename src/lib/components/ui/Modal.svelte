<script lang="ts">
  export let open = false;
  export let title = '';
  export let onClose: () => void = () => {};
</script>

{#if open}
  <div class="modal-overlay" role="dialog" aria-modal="true" onclick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
    <div class="modal">
      <div class="modal-header">
        <h3>{title}</h3>
        <button class="close-btn" aria-label="Close" onclick={onClose}>
          ✕
        </button>
      </div>
      <div class="modal-content">
        <slot />
      </div>
    </div>
  </div>
{/if}

<style>
.modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,.5); display: flex; align-items: center; justify-content: center; z-index: 1000; }
.modal { background: var(--bg-primary); border-radius: 8px; width: 90%; max-width: 560px; box-shadow: 0 10px 25px rgba(0,0,0,.15); overflow: hidden; }
.modal-header { display: flex; justify-content: space-between; align-items: center; padding: 1rem 1.25rem; border-bottom: 1px solid var(--border-color); }
.modal-content { padding: 1rem 1.25rem; }
.close-btn { background: none; border: 1px solid var(--border-color); border-radius: 6px; padding: .25rem .5rem; cursor: pointer; color: var(--text-secondary); }
.close-btn:hover { background: var(--bg-hover); }
</style>


