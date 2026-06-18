<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import RoundButton from "../ui/RoundButton.svelte";
	import Modal from "../ui/Modal.svelte";
	import { t } from "$lib/i18n";

	export let open = false;
	
	const dispatch = createEventDispatcher<{
		close: void;
		confirm: string;
	}>();

	let mcpJson = "";
	let isLoading = false;
	let errors: Record<string, string> = {};
	
	// Modal 引用
	let modalRef: Modal;

	function handleClose() {
		modalRef?.handleClose();
	}
	
	function onModalClose() {
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
			modalRef?.handleClose();
		} catch (error) {
			console.error("Failed to create provider:", error);
		} finally {
			isLoading = false;
		}
	}

	function validate() {
		errors = {};

		if (!mcpJson.trim()) {
			errors.name = t("provider.validateMcpJson");
		}

		return Object.keys(errors).length === 0;
	}
</script>

<Modal bind:this={modalRef} {open} onClose={onModalClose} showCloseButton={false}>
	<div
		class="min-w-lg max-w-xl h-[80vh] overflow-hidden flex flex-col"
	>
		<div class="flex items-center justify-between px-6 py-4">
			<h2 class="font-normal text-base-content">{t("provider.editMcpJsonTitle")}</h2>
		</div>

		<div class="flex-1 px-6">
			<textarea
				class="w-full h-full min-h-40 border border-[var(--hairline)] rounded-md p-2 resize-none bg-base-300 text-base-content"
				placeholder={t("provider.mcpJsonPlaceholder")}
				bind:value={mcpJson}
			></textarea>
		</div>

		<!-- 底部按钮 -->
		<div class="flex items-center justify-end gap-3 px-6 py-3">
			<RoundButton
				customClass="w-18"
				label={t("common.cancel")}
				variant="secondary"
				onclick={handleClose}
			></RoundButton>
			<RoundButton
				customClass="w-18"
				label={t("provider.confirm")}
				onclick={handleConfirm}
				disabled={isLoading}
			></RoundButton>
		</div>
	</div>
</Modal>
