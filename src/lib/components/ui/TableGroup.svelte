<script lang="ts">
  interface Props {
    title?: string;
    children?: any;
  }

  let { title, children }: Props = $props();
</script>

<div class="bg-bg-secondary rounded-[20px] overflow-hidden">
  <div class="">
    {@render children?.()}
  </div>
</div>
