/**
 * 应用更新状态管理 - Svelte 5
 *
 * 基于 @tauri-apps/plugin-updater 实现自动 / 手动检查更新、下载安装并重启。
 * 「发现更新」通过 Tauri 事件在多窗口间广播，使设置窗口的手动检查也能点亮
 * 主窗口侧边栏的更新入口。自动检查偏好用 localStorage 持久化（与 theme 一致）。
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
  | 'idle' // 未检查 / 已是最新
  | 'checking' // 正在检查
  | 'available' // 发现新版本
  | 'downloading' // 正在下载安装
  | 'error'; // 检查或下载失败

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

  // check() 返回的 Update 句柄；非响应式，仅在当前窗口有效
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
  /** 是否存在可更新版本（含下载中），用于侧边栏入口显隐 */
  get hasUpdate() {
    return this.state.status === 'available' || this.state.status === 'downloading';
  }
  /** 下载进度 0..1（contentLength 未知时恒为 0） */
  get progress() {
    if (this.state.contentLength <= 0) return 0;
    return Math.min(1, this.state.downloaded / this.state.contentLength);
  }

  /** 读取当前版本与 autoCheck 偏好；两个窗口都会调用，幂等 */
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

  /** 稍后提醒：关闭弹框，保留侧边栏入口 */
  remindLater(): void {
    this.state.dialogOpen = false;
  }

  /**
   * 主窗口启动时调用：监听跨窗口的「发现更新」事件；若开启自动检查则静默检查一次。
   * 返回取消监听的清理函数。
   */
  async startAutoCheck(): Promise<UnlistenFn> {
    const unlisten = await listen<UpdateInfo>(UPDATE_AVAILABLE_EVENT, (event) => {
      // 来自其它窗口（如设置窗口手动检查）的更新通知：点亮入口，但不自动弹框
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
   * 检查更新。
   * @param notifyNoUpdate 无更新 / 出错时是否 toast 提示（手动检查为 true）
   * @param openOnFound 发现更新时是否自动打开弹框（默认 true）
   * @param broadcast 发现更新时是否向其它窗口广播（默认 true）
   * @returns 是否发现可更新版本
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

  /** 下载并安装更新，完成后重启应用 */
  async startUpdate(): Promise<void> {
    if (this.state.status === 'downloading') return;

    // 当前窗口没有句柄（如仅通过跨窗口事件得知更新），重新检查以获取
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
      // 安装完成，重启进入新版本
      await relaunch();
    } catch (error) {
      this.state.status = 'available';
      this.state.error = error instanceof Error ? error.message : String(error);
      console.error('Update install failed:', error);
      toastActions.error('更新失败', { hint: this.state.error ?? undefined });
    }
  }
}

// 导出单例实例
export const updateState = new UpdateState();
