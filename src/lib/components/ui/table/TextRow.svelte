<script lang="ts">
  import { Eye, EyeOff } from "@lucide/svelte";
  import IconButton from "../IconButton.svelte";
  import TableBaseRow from "./TableBaseRow.svelte";

  export let layout: "horizontal" | "vertical" = "horizontal";
  export let label: string;
  export let value: string = "";
  export let placeholder: string = "请输入";
  export let readonly: boolean = false;
  export let isPassword: boolean = false;

  let showPassword = false;
  $: inputType = isPassword ? (showPassword ? "text" : "password") : "text";

  function togglePassword() {
    showPassword = !showPassword;
  }
</script>

{#if layout === "horizontal"}
  <TableBaseRow {label} {layout} py="2">
    <!-- 输入框 -->
    <div class="flex flex-col items-end flex-1">
      <input
        bind:value
        {placeholder}
        {readonly}
        class="w-full text-sm text-right text-base-content border-none outline-none p-1"
        class:cursor-not-allowed={readonly}
        class:opacity-60={readonly}
        on:input
      />
    </div>
  </TableBaseRow>
{:else}
  <TableBaseRow {label} {layout}>
    <div class="flex flex-row bg-base-100 rounded-lg overflow-hidden">
      <input
        type={inputType}
        bind:value
        {placeholder}
        {readonly}
        class="w-full bg-base-100 text-base text-left text-base-content border-none outline-none px-2 py-1"
        class:cursor-not-allowed={readonly}
        class:opacity-60={readonly}
        on:input
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
  </TableBaseRow>
{/if}
