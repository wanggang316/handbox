<script lang="ts">
  interface Props {
    status: 'enabled' | 'disabled' | 'pending' | 'error';
    text: string;
  }

  let { status, text }: Props = $props();

  const statusStyles = {
    enabled: 'bg-green-100 text-green-800 border-green-200',
    disabled: 'bg-slate-100 text-slate-600 border-slate-200',
    pending: 'bg-yellow-100 text-yellow-800 border-yellow-200',
    error: 'bg-red-100 text-red-800 border-red-200'
  };
</script>

<span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium border {statusStyles[status]}">
  {text}
</span>