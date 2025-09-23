<script lang="ts">
  interface Props {
    title?: string;
    children?: any;
  }

  let { title, children }: Props = $props();
</script>

{#if title}
  <div class="flex flex-col pt-2">
    <div class="text-base-content/80 text-xs my-1 mx-2">
      {title}
    </div>
    <div class="relative flex-1">
      <div
        class="absolute inset-0 bg-base-200 rounded-[20px] pointer-events-none"
      ></div>
      <div class="relative table-group rounded-[20px]">
        {@render children?.()}
      </div>
    </div>
  </div>
{:else}
  <div class="relative">
    <div
      class="absolute inset-0 bg-base-200 rounded-[20px] pointer-events-none"
    ></div>
    <div class="relative table-group rounded-[20px]">
      {@render children?.()}
    </div>
  </div>
{/if}

<style>
  .table-group :global(> *:not(:last-child)) {
    position: relative;
  }

  .table-group :global(> *:not(:last-child)::after) {
    content: "";
    position: absolute;
    bottom: 0;
    left: 1.5rem;
    right: 1.5rem;
    height: 1px;
    background-color: var(--base-300);
  }
</style>
