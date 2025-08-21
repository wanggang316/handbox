<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import RoundButton from "../ui/RoundButton.svelte";

	const dispatch = createEventDispatcher<{
		close: void;
		confirm: string;
	}>();

	let mcpJson = "";
	let isLoading = false;
	let errors: Record<string, string> = {};

	function handleClose() {
		dispatch("close");
	}

	async function handleConfirm() {
		if (!validate()) {
			console.log("errors", errors);
			return;
		}

		isLoading = true;

		console.log("mcpJson", mcpJson);
		try {
			dispatch("confirm", mcpJson);
		} catch (error) {
			console.error("Failed to create provider:", error);
		} finally {
			isLoading = false;
		}
	}

	function validate() {
		errors = {};

		if (!mcpJson.trim()) {
			errors.name = "请输入MCP服务器配置";
		}

		return Object.keys(errors).length === 0;
	}
</script>

<div
	class="fixed inset-0 transition-colors flex items-center justify-center z-10005 p-4"
>
	<div
		class="bg-white rounded-2xl w-full max-w-lg max-h-[80vh] overflow-hidden flex flex-col shadow-2xl"
	>
		<div class="flex items-center justify-between px-6 py-4">
			<h2 class="font-normal text-text-primary">编辑MCP服务器</h2>
		</div>

		<div class="flex-1 px-6">
			<textarea
				class="w-full h-full min-h-110 border-1 border-gray-200 rounded-md p-2 resize-none"
				placeholder="请输入MCP服务器配置..."
				bind:value={mcpJson}
			></textarea>
		</div>

		<!-- 底部按钮 -->
		<div class="flex items-center justify-end gap-3 px-6 py-3">
			<RoundButton
				customClass="w-18"
				label="取消"
				bgColor="bg-gray-200"
				textColor="text-gray-600"
				hoverColor="hover:text-gray-800"
				on:click={handleClose}
			></RoundButton>
			<RoundButton
				customClass="w-18"
				label="确认"
				on:click={handleConfirm}
				disabled={isLoading}
			></RoundButton>
		</div>
	</div>
</div>
