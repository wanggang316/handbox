<script lang="ts">
  import { Eye, EyeOff } from "@lucide/svelte";
  import IconButton from "../IconButton.svelte";
  import TableBaseRow from "./TableBaseRow.svelte";

  interface Props {
    layout?: "horizontal" | "vertical";
    label: string;
    value?: string;
    placeholder?: string;
    readonly?: boolean;
    isPassword?: boolean;
    disabled?: boolean;
    required?: boolean;
    error?: string;
  }

  let {
    layout = "horizontal",
    label,
    value = $bindable(""),
    placeholder = "请输入",
    readonly = false,
    isPassword = false,
    disabled = false,
    required = false,
    error = "",
  }: Props = $props();

  const id = `txtrow-${Math.random().toString(36).slice(2)}`;
  const errorId = `${id}-error`;

  let showPassword = $state(false);
  const inputType = $derived(
    isPassword ? (showPassword ? "text" : "password") : "text"
  );

  function togglePassword() {
    showPassword = !showPassword;
  }
</script>

{#if layout === "horizontal"}
  <TableBaseRow label={`${label}${required ? " *" : ""}`} {layout} py="2">
    <!-- 输入框 -->
    <div class="flex flex-col items-end flex-1">
      <input
        {id}
        bind:value
        {placeholder}
        {readonly}
        {disabled}
        {required}
        aria-required={required ? "true" : undefined}
        aria-invalid={error ? "true" : undefined}
        aria-describedby={error ? errorId : undefined}
        class="w-full text-sm text-right text-base-content border-none p-1"
        class:cursor-not-allowed={readonly || disabled}
        class:opacity-60={readonly || disabled}
        class:text-error={!!error}
      />
      {#if error}
        <p id={errorId} class="text-xs text-error mt-1">{error}</p>
      {/if}
    </div>
  </TableBaseRow>
{:else}
  <TableBaseRow label={`${label}${required ? " *" : ""}`} {layout}>
    <div
      class="flex flex-row bg-base-100 rounded-lg overflow-hidden"
      class:border={!!error}
      class:border-error={!!error}
    >
      <input
        {id}
        type={inputType}
        bind:value
        {placeholder}
        {readonly}
        {disabled}
        {required}
        aria-required={required ? "true" : undefined}
        aria-invalid={error ? "true" : undefined}
        aria-describedby={error ? errorId : undefined}
        class="w-full bg-base-100 text-base text-left text-base-content border-none px-2 py-1"
        class:cursor-not-allowed={readonly || disabled}
        class:opacity-60={readonly || disabled}
      />

      <div
        class="px-1 flex items-center justify-center"
        class:hidden={!isPassword}
      >
        <IconButton
          icon={showPassword ? Eye : EyeOff}
          onclick={togglePassword}
        />
      </div>
    </div>
    {#if error}
      <p id={errorId} class="text-xs text-error mt-1">{error}</p>
    {/if}
  </TableBaseRow>
{/if}
