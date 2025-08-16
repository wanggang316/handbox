<script lang="ts">
  interface Props {
    title?: string;
    children?: any;
  }

  let { title, children }: Props = $props();
</script>

<div class="bg-bg-secondary rounded-[20px] overflow-hidden">
  <div class="table-group">
    {@render children?.()}
  </div>
</div>

<style>
  .table-group :global(> *:not(:last-child)) {
    position: relative;
  }
  
  .table-group :global(> *:not(:last-child)::after) {
    content: '';
    position: absolute;
    bottom: 0;
    left: 1.5rem;
    right: 1.5rem;
    height: 1px;
    background-color: var(--border-color);
  }
</style>
