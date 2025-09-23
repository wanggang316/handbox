<script lang="ts">
	import { ChevronRight } from "@lucide/svelte";
	import Toggle from "../Toggle.svelte";
	import TableBaseRow from "./TableBaseRow.svelte";

	interface Props {
		label: string;
		value?: string;
		clickable?: boolean;
		onclick?: () => void;
	}

	let { label, value = $bindable(), clickable = true, onclick = $bindable() }: Props = $props();
</script>

<button
	class="w-full {clickable ? 'hover:bg-base-300' : ''} group"
	{onclick}
	onkeydown={(e) => e.key === "Enter" && onclick?.()}
>
	<TableBaseRow {label}>
		<div class="flex flex-row items-center justify-between">
			{#if value}
				<div class="text-sm text-base-content/80 text-right px-2">
					{value}
				</div>
			{/if}
			<ChevronRight
				size="16"
				class="text-base-content/50 group-hover:text-base-content transition-colors duration-75"
			/>
		</div>
	</TableBaseRow>
</button>
