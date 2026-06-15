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
    placeholder = "",
    rows = 4,
    disabled = false,
    readonly = false,
    maxlength,
    minlength,
    required = false,
    id,
    name,

    showCharCount = false,
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
    class="w-full px-3 py-2 border border-[var(--hairline)] rounded-md resize-none
           focus:border-transparent
           font-mono text-sm text-base-content bg-base-300
           scrollbar-thin scrollbar-thumb-base-300 scrollbar-track-base-200
           hover:scrollbar-thumb-base-300/80
           disabled:opacity-50 disabled:cursor-not-allowed
           readonly:opacity-80"
  ></textarea>

  {#if showCharCount}
    <div class="text-xs text-base-content/70 text-left">
      {#if maxlength}
        {value.length} / {maxlength}
      {:else}
        字符数: {value.length}
      {/if}
    </div>
  {/if}
</div>
