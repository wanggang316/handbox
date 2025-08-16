<script lang="ts">
  interface Props {
    value: string;
    placeholder?: string;
    rows?: number;
    disabled?: boolean;
    readonly?: boolean;
    maxlength?: number;
    minlength?: number;
    required?: boolean;
    id?: string;
    name?: string;

    showCharCount?: boolean;
  }

  let { 
    value = $bindable(),
    placeholder = '',
    rows = 4,
    disabled = false,
    readonly = false,
    maxlength,
    minlength,
    required = false,
    id,
    name,

    showCharCount = false
  }: Props = $props();

  function handleInput(event: Event) {
    const target = event.target as HTMLTextAreaElement;
    value = target.value;
  }
</script>

<div class="space-y-2">
  <textarea
    {id}
    {name}
    {placeholder}
    {rows}
    {disabled}
    {readonly}
    {maxlength}
    {minlength}
    {required}

    {value}
    oninput={handleInput}
    class="w-full px-3 py-2 border border-gray-300 rounded-md resize-none 
           focus:outline-none focus:ring-2 focus:ring-bg-accent focus:border-transparent 
           font-mono text-sm bg-bg-primary
           scrollbar-thin scrollbar-thumb-gray-300 scrollbar-track-gray-100 
           hover:scrollbar-thumb-gray-400
           disabled:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-50
           readonly:bg-gray-50"
  ></textarea>
  
  {#if showCharCount}
    <div class="text-xs text-gray-500 text-left">
      {#if maxlength}
        {value.length} / {maxlength}
      {:else}
        字符数: {value.length}
      {/if}
    </div>
  {/if}
</div>
