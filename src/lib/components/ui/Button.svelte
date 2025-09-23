<script lang="ts">
  export let variant: 'primary' | 'secondary' | 'gray' | 'danger' | 'ghost' | 'clear' = 'primary';
  export let size: 'sm' | 'md' = 'md';
  export let disabled = false;
  export let type: 'button' | 'submit' | 'reset' = 'button';
  export let customClass: string = '';
</script>

<button class={`btn ${variant} ${size} ${customClass}`} {type} {disabled} on:click>
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
      /* transition: opacity 0.2s, background-color 0.2s; */
    }
    .btn:disabled {
      opacity: 0.6;
      cursor: not-allowed;
    }
    .md { padding: 0.5rem 1rem; }
    .sm { padding: 0.25rem 0.5rem; font-size: 0.875rem; }
    .primary { background: var(--primary); color: var(--primary-content); }
    .secondary { background: var(--base-200); color: var(--base-content); border: 1px solid var(--base-300); }
    .gray { background: var(--base-200); color: color-mix(in oklch, var(--base-content) 80%, transparent); border: 1px solid var(--base-300); }
    .danger { background: var(--error); color: var(--error-content); }
    .ghost { background: transparent; color: color-mix(in oklch, var(--base-content) 80%, transparent); border: 1px solid var(--base-300); }
    .clear { background: transparent; color: var(--base-content); border: none; }
    .btn:hover:not(:disabled) { opacity: 0.9; }
    .btn.gray:hover:not(:disabled) { background: var(--base-300); color: var(--base-content); opacity: 1; }
    .btn.ghost:hover:not(:disabled) { background: var(--base-300); color: var(--base-content); opacity: 1; }
    .btn.clear:hover:not(:disabled) { background: var(--base-300); color: var(--base-content); opacity: 1; }
  </style>
</button>
