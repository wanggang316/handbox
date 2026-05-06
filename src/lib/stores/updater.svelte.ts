import { check, type DownloadEvent, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

type UpdaterState = {
	checked: boolean;
	checking: boolean;
	installing: boolean;
	hasUpdate: boolean;
	latestVersion: string;
	update: Update | null;
	error: string;
};

function createUpdaterStore() {
	const state = $state<UpdaterState>({
		checked: false,
		checking: false,
		installing: false,
		hasUpdate: false,
		latestVersion: '',
		update: null,
		error: ''
	});

	let checkPromise: Promise<void> | null = null;

	const getErrorMessage = (error: unknown, fallback: string): string => {
		if (error instanceof Error && error.message) return error.message;
		if (typeof error === 'string' && error.trim()) return error;
		try {
			const text = JSON.stringify(error);
			return text && text !== '{}' ? text : fallback;
		} catch {
			return fallback;
		}
	};

	async function ensureChecked(force = false): Promise<void> {
		if (checkPromise) {
			await checkPromise;
			return;
		}

		if (!force && (state.checked || state.checking)) {
			return;
		}

		const task = (async () => {
			state.checking = true;
			state.error = '';
			try {
				const update = await check();
				state.checked = true;
				state.checking = false;
				state.hasUpdate = Boolean(update);
				state.latestVersion = update?.version ?? '';
				state.update = update ?? null;
			} catch (error) {
				state.checked = true;
				state.checking = false;
				state.error = getErrorMessage(error, 'Failed to check for update');
				throw error;
			}
		})();

		checkPromise = task;
		try {
			await task;
		} finally {
			checkPromise = null;
		}
	}

	async function installAvailable(
		onDownloadEvent?: (event: DownloadEvent) => void
	): Promise<void> {
		if (state.installing) return;

		if (!state.update) {
			await ensureChecked(true);
		}
		if (!state.update) return;

		state.installing = true;
		state.error = '';
		try {
			await state.update.downloadAndInstall((event: DownloadEvent) => {
				onDownloadEvent?.(event);
			});
			await relaunch();
		} catch (error) {
			state.installing = false;
			state.error = getErrorMessage(error, 'Failed to install update');
			throw error;
		}
	}

	return {
		get state() {
			return state;
		},
		ensureChecked,
		installAvailable
	};
}

export const updaterStore = createUpdaterStore();
