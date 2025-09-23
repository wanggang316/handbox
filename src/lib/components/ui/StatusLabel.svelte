<script lang="ts">
  interface Props {
    status: 'enabled' | 'disabled' | 'idle' | 'error';
    text: string;
  }

  let { status, text }: Props = $props();

  const statusStyles = {
    enabled: 'bg-success/10 text-success border-success/30',
    disabled: 'bg-warning/10 text-warning border-warning/30',
    idle: 'bg-base-200 text-base-content/70 border-base-300',
    error: 'bg-error/10 text-error border-error/30'
  };
</script>

<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium border {statusStyles[status]}">
  {text}
</span>
