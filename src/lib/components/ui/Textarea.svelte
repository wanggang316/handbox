<script lang="ts">
  import { t } from "$lib/i18n";
  import { getFormField } from "./FormField.svelte";

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
    /**
     * Identifier-style input: turns off the platform's autocapitalize /
     * autocorrect / spellcheck so the value is stored exactly as typed. Needed
     * for anything executed or matched literally — a script, a path — where
     * macOS would otherwise capitalize the first letter and silently break it.
     */
    literal?: boolean;

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
    literal = false,

    showCharCount = false,
  }: Props = $props();

  // Inside a FormField the field supplies id / aria / error state; standalone use is unchanged.
  const ff = getFormField();
  const controlId = $derived(ff ? ff.id : id);
  const invalid = $derived(ff ? ff.invalid : false);
  const describedby = $derived(ff ? ff.describedby : undefined);

  function handleInput(event: Event) {
    const target = event.target as HTMLTextAreaElement;
    value = target.value;
  }
</script>

<div class="space-y-2">
  <textarea
    id={controlId}
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
    aria-invalid={invalid ? "true" : undefined}
    aria-describedby={describedby}
    autocapitalize={literal ? "off" : undefined}
    spellcheck={literal ? false : undefined}
    {...literal ? { autocorrect: "off" } : {}}
    class="field w-full px-3 py-2 resize-none font-mono text-sm
           scrollbar-thin scrollbar-thumb-base-300 scrollbar-track-base-200
           hover:scrollbar-thumb-base-300/80
           readonly:opacity-80"
    class:is-error={invalid}
  ></textarea>

  {#if showCharCount}
    <div class="text-xs text-base-content/70 text-left">
      {#if maxlength}
        {value.length} / {maxlength}
      {:else}
        {t("ui.charCount", { count: value.length })}
      {/if}
    </div>
  {/if}
</div>
