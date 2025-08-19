<script lang="ts">
  interface Props {
    title?: string;
    children?: any;
  }

  let { title, children }: Props = $props();
</script>

<div class="relative rounded-[20px]">
  <div class="absolute inset-0 bg-bg-secondary rounded-[20px] pointer-events-none"></div>
  <div class="relative table-group rounded-[20px]">
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
