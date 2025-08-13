<script lang="ts">
  export let variant: 'primary' | 'secondary' | 'danger' | 'ghost' = 'primary';
  export let size: 'sm' | 'md' = 'md';
  export let disabled = false;
  export let type: 'button' | 'submit' | 'reset' = 'button';
</script>

<button class={`btn ${variant} ${size}`} {type} {disabled}>
  <slot />
  <style>
    .btn {
      border: none;
      border-radius: 6px;
      cursor: pointer;
      font-weight: 500;
      display: inline-flex;
      align-items: center;
      gap: 0.5rem;
      transition: opacity 0.2s;
    }
    .btn:disabled {
      opacity: 0.6;
      cursor: not-allowed;
    }
    .md { padding: 0.5rem 1rem; }
    .sm { padding: 0.25rem 0.5rem; font-size: 0.875rem; }
    .primary { background: var(--bg-accent); color: var(--text-accent); }
    .secondary { background: var(--bg-secondary); color: var(--text-primary); border: 1px solid var(--border-color); }
    .danger { background: #ef4444; color: #fff; }
    .ghost { background: transparent; color: var(--text-secondary); border: 1px solid var(--border-color); }
    .btn:hover:not(:disabled) { opacity: 0.9; }
  </style>
</button>


