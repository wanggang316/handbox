/**
 * App-update state - Svelte 5.
 *
 * Auto / manual update check, download-install, and relaunch, built on
 * @tauri-apps/plugin-updater. "Update available" is broadcast across windows
 * via a Tauri event, so a manual check in the settings window also lights up
 * the main window's sidebar entry. The auto-check preference persists in
 * localStorage (same as theme).
 */

import {
  check,
  type Update,
  type DownloadEvent,
} from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { getVersion } from '@tauri-apps/api/app';
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';
import { toastActions } from './toast.svelte';

export type UpdateStatus =
  | 'idle' // not checked / up to date
  | 'checking'
  | 'available' // new version found
  | 'downloading' // downloading and installing
  | 'error'; // check or download failed

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  body?: string;
  date?: string;
}

const AUTO_CHECK_KEY = 'update.autoCheck';
const UPDATE_AVAILABLE_EVENT = 'update://available';

interface UpdateStateData {
  status: UpdateStatus;
  info: UpdateInfo | null;
  currentVersion: string;
  dialogOpen: boolean;
  downloaded: number;
  contentLength: number;
  autoCheck: boolean;
  error: string | null;
}

class UpdateState {
  private state = $state<UpdateStateData>({
    status: 'idle',
    info: null,
    currentVersion: '',
    dialogOpen: false,
    downloaded: 0,
    contentLength: 0,
    autoCheck: true,
    error: null,
  });

  // Update handle returned by check(); non-reactive, valid in this window only.
  private handle: Update | null = null;
  private loaded = false;

  // ---- getters ----
  get status() {
    return this.state.status;
  }
  get info() {
    return this.state.info;
  }
  get currentVersion() {
    return this.state.currentVersion;
  }
  get dialogOpen() {
    return this.state.dialogOpen;
  }
  get autoCheck() {
    return this.state.autoCheck;
  }
  get error() {
    return this.state.error;
  }
  get contentLength() {
    return this.state.contentLength;
  }
  /** Whether an update exists (including while downloading); drives the sidebar entry. */
  get hasUpdate() {
    return this.state.status === 'available' || this.state.status === 'downloading';
  }
  /** Download progress 0..1 (always 0 when contentLength is unknown). */
  get progress() {
    if (this.state.contentLength <= 0) return 0;
    return Math.min(1, this.state.downloaded / this.state.contentLength);
  }

  /** Read the current version and autoCheck preference; called by both windows, idempotent. */
  async load(): Promise<void> {
    if (this.loaded) return;
    this.loaded = true;
    if (typeof localStorage !== 'undefined') {
      const saved = localStorage.getItem(AUTO_CHECK_KEY);
      this.state.autoCheck = saved === null ? true : saved === 'true';
    }
    try {
      this.state.currentVersion = await getVersion();
    } catch (error) {
      console.error('Failed to get app version:', error);
    }
  }

  setAutoCheck(enabled: boolean): void {
    this.state.autoCheck = enabled;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem(AUTO_CHECK_KEY, String(enabled));
    }
  }

  openDialog(): void {
    this.state.dialogOpen = true;
  }

  closeDialog(): void {
    this.state.dialogOpen = false;
  }

  /** Remind later: close the dialog but keep the sidebar entry. */
  remindLater(): void {
    this.state.dialogOpen = false;
  }

  /**
   * Called at main-window startup: listen for the cross-window
   * "update available" event and, if auto-check is on, check once silently.
   * Returns the unlisten cleanup function.
   */
  async startAutoCheck(): Promise<UnlistenFn> {
    const unlisten = await listen<UpdateInfo>(UPDATE_AVAILABLE_EVENT, (event) => {
      // Update notice from another window (e.g. manual check in settings):
      // light up the entry but do not auto-open the dialog.
      if (this.state.status === 'downloading') return;
      this.state.info = event.payload;
      this.state.status = 'available';
    });

    if (this.state.autoCheck) {
      this.checkForUpdate({ notifyNoUpdate: false, openOnFound: true }).catch((error) =>
        console.error('Auto update check failed:', error)
      );
    }

    return unlisten;
  }

  /**
   * Check for an update.
   * @param notifyNoUpdate toast when up to date / on error (true for manual checks)
   * @param openOnFound auto-open the dialog when an update is found (default true)
   * @param broadcast broadcast to other windows when found (default true)
   * @returns whether an update was found
   */
  async checkForUpdate(opts?: {
    notifyNoUpdate?: boolean;
    openOnFound?: boolean;
    broadcast?: boolean;
  }): Promise<boolean> {
    const notifyNoUpdate = opts?.notifyNoUpdate ?? false;
    const openOnFound = opts?.openOnFound ?? true;
    const broadcast = opts?.broadcast ?? true;

    if (this.state.status === 'checking' || this.state.status === 'downloading') {
      return this.state.status === 'downloading';
    }

    this.state.status = 'checking';
    this.state.error = null;
    try {
      const update = await check();
      if (!update) {
        this.handle = null;
        this.state.status = 'idle';
        this.state.info = null;
        if (notifyNoUpdate) toastActions.success('已是最新版本');
        return false;
      }

      this.handle = update;
      this.state.info = {
        version: update.version,
        currentVersion: update.currentVersion,
        body: update.body,
        date: update.date,
      };
      this.state.status = 'available';

      if (broadcast) {
        emit(UPDATE_AVAILABLE_EVENT, { ...this.state.info }).catch((error) =>
          console.error('Failed to broadcast update event:', error)
        );
      }
      if (openOnFound) this.state.dialogOpen = true;
      return true;
    } catch (error) {
      this.state.status = 'error';
      this.state.error = error instanceof Error ? error.message : String(error);
      console.error('Update check failed:', error);
      if (notifyNoUpdate) {
        toastActions.error('检查更新失败', { hint: this.state.error ?? undefined });
      }
      return false;
    }
  }

  /** Download and install the update, then relaunch the app. */
  async startUpdate(): Promise<void> {
    if (this.state.status === 'downloading') return;

    // No handle in this window (e.g. the update was learned via the
    // cross-window event): re-check to obtain one.
    if (!this.handle) {
      const ok = await this.checkForUpdate({
        notifyNoUpdate: false,
        openOnFound: false,
        broadcast: false,
      });
      if (!ok || !this.handle) {
        this.state.status = 'error';
        this.state.error = '无法获取更新包';
        toastActions.error('无法获取更新包');
        return;
      }
    }

    this.state.status = 'downloading';
    this.state.downloaded = 0;
    this.state.contentLength = 0;
    this.state.error = null;
    try {
      await this.handle.downloadAndInstall((event: DownloadEvent) => {
        switch (event.event) {
          case 'Started':
            this.state.contentLength = event.data.contentLength ?? 0;
            this.state.downloaded = 0;
            break;
          case 'Progress':
            this.state.downloaded += event.data.chunkLength;
            break;
          case 'Finished':
            this.state.downloaded = this.state.contentLength;
            break;
        }
      });
      // Installed; relaunch into the new version.
      await relaunch();
    } catch (error) {
      this.state.status = 'available';
      this.state.error = error instanceof Error ? error.message : String(error);
      console.error('Update install failed:', error);
      toastActions.error('更新失败', { hint: this.state.error ?? undefined });
    }
  }
}

export const updateState = new UpdateState();
