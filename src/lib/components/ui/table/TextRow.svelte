<script lang="ts">
    import TableBaseRow from "./TableBaseRow.svelte";

    export let label: string;
    export let value: string = "";
    export let placeholder: string = "请输入";
    export let readonly: boolean = false;
</script>

<TableBaseRow {label} layout="horizontal">
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
