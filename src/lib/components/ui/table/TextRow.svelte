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
    <TableBaseRow {label} {layout}>
        <!-- 输入框 -->
        <div class="flex flex-col items-end">
            <input
                bind:value
                {placeholder}
                {readonly}
                class="w-full bg-transparent text-base text-right text-text-primary border-none outline-none p-1"
                class:cursor-not-allowed={readonly}
                class:text-[#b3b3b3]={readonly}
            />
        </div>
    </TableBaseRow>
{:else}
    <TableBaseRow {label} {layout}>
        <div class="flex flex-row bg-white rounded-lg overflow-hidden">
            <input
                type={inputType}
                bind:value
                {placeholder}
                {readonly}
                class="w-full bg-white text-base text-left text-text-primary border-none outline-none px-2 py-1"
                class:cursor-not-allowed={readonly}
                class:text-[#b3b3b3]={readonly}
            />

            <div class="px-1 flex items-center justify-center" class:hidden={!isPassword}>
                <IconButton
                    icon={showPassword ? Eye : EyeOff}
                    on:click={togglePassword}
                />
            </div>
        </div>
    </TableBaseRow>
{/if}
