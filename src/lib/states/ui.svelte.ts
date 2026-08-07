/**
 * UI state - Svelte 5.
 */

import type { Theme, Language } from "../types";
import { isTauriEnvironment } from "../utils/tauri";

const LAST_AGENT_SESSION_ID_KEY = "lastAgentSessionId";
const LANGUAGE_KEY = "language";

const SUPPORTED_LANGUAGES: ReadonlySet<Language> = new Set<Language>([
  "zh-CN",
  "en-US",
]);

function loadPersistedLanguage(): Language {
  if (typeof localStorage === "undefined") return "zh-CN";
  const saved = localStorage.getItem(LANGUAGE_KEY);
  return saved && SUPPORTED_LANGUAGES.has(saved as Language)
    ? (saved as Language)
    : "zh-CN";
}

function loadPersistedLastAgentSessionId(): string | null {
  if (typeof localStorage === "undefined") return null;
  return localStorage.getItem(LAST_AGENT_SESSION_ID_KEY) || null;
}

export interface Notification {
  id: string;
  type: "info" | "success" | "warning" | "error";
  title: string;
  message?: string;
  duration?: number;
  actions?: Array<{ label: string; action: () => void }>;
}

interface UIStateData {
  sidebarOpen: boolean;
  sidebarWidth: number;
  currentPage: string;
  modals: Record<string, boolean>;
  notifications: Notification[];
  theme: Theme;
  language: Language;
  globalLoading: boolean;
  lastAgentSessionId: string | null;
}

class UIState {
  private state = $state<UIStateData>({
    sidebarOpen: true,
    sidebarWidth: 240,
    currentPage: "chat",
    modals: {},
    notifications: [],
    theme: "system",
    language: loadPersistedLanguage(),
    globalLoading: false,
    lastAgentSessionId: loadPersistedLastAgentSessionId(),
  });

  // Getters
  get sidebarOpen() {
    return this.state.sidebarOpen;
  }

  get sidebarWidth() {
    return this.state.sidebarWidth;
  }

  get currentPage() {
    return this.state.currentPage;
  }

  get modals() {
    return this.state.modals;
  }

  get notifications() {
    return this.state.notifications;
  }

  get theme() {
    return this.state.theme;
  }

  get language() {
    return this.state.language;
  }

  get globalLoading() {
    return this.state.globalLoading;
  }

  get lastAgentSessionId() {
    return this.state.lastAgentSessionId;
  }

  get isDarkMode(): boolean {
    const theme = this.state.theme;
    if (theme === "dark") return true;
    if (theme === "light") return false;

    // "system": follow the OS preference (browser environment only).
    if (typeof window !== "undefined") {
      return window.matchMedia("(prefers-color-scheme: dark)").matches;
    }
    return false;
  }

  // Actions
  setSidebarOpen(open: boolean) {
    this.state.sidebarOpen = open;
  }

  setSidebarWidth(width: number) {
    this.state.sidebarWidth = width;
  }

  setCurrentPage(page: string) {
    this.state.currentPage = page;
  }

  setModals(modals: Record<string, boolean>) {
    this.state.modals = modals;
  }

  setNotifications(notifications: Notification[]) {
    this.state.notifications = notifications;
  }

  setThemeState(theme: Theme) {
    this.state.theme = theme;
  }

  setLanguageState(language: Language) {
    this.state.language = language;
  }

  setGlobalLoading(loading: boolean) {
    this.state.globalLoading = loading;
  }

  toggleSidebar(): void {
    this.state.sidebarOpen = !this.state.sidebarOpen;
  }

  openModal(modalId: string): void {
    this.state.modals = { ...this.state.modals, [modalId]: true };
  }

  closeModal(modalId: string): void {
    this.state.modals = { ...this.state.modals, [modalId]: false };
  }

  toggleModal(modalId: string): void {
    this.state.modals = {
      ...this.state.modals,
      [modalId]: !this.state.modals[modalId],
    };
  }

  showNotification(notification: Omit<Notification, "id">): string {
    const id = crypto.randomUUID();
    const newNotification: Notification = {
      id,
      duration: 5000,
      ...notification,
    };

    this.state.notifications = [...this.state.notifications, newNotification];

    // Auto-dismiss.
    if (newNotification.duration && newNotification.duration > 0) {
      setTimeout(() => {
        this.removeNotification(id);
      }, newNotification.duration);
    }

    return id;
  }

  removeNotification(id: string): void {
    this.state.notifications = this.state.notifications.filter(
      (n) => n.id !== id,
    );
  }

  clearNotifications(): void {
    this.state.notifications = [];
  }

  setTheme(newTheme: Theme): void {
    this.state.theme = newTheme;

    if (typeof localStorage !== "undefined") {
      const current = localStorage.getItem("theme");
      if (current !== newTheme) {
        localStorage.setItem("theme", newTheme);
      }
    }

    // Update the HTML data-theme attribute to match the CSS selectors.
    if (typeof document !== "undefined") {
      const resolved =
        newTheme === "system"
          ? window.matchMedia("(prefers-color-scheme: dark)").matches
            ? "dark"
            : "light"
          : newTheme;
      document.documentElement.setAttribute("data-theme", resolved);
      // Keep the UA color-scheme in step with data-theme. app.html sets it at
      // boot; without this re-sync, a mid-session switch leaves UA-default
      // text, form controls, and scrollbars rendering for the previous theme.
      document.documentElement.style.colorScheme = resolved;
    }

    // Sync the native window appearance: data-theme only affects webview
    // content, while the NSWindow still follows the OS appearance — with
    // "app light + system dark" macOS draws the window border and title-bar
    // overlay dark. Aligning the window theme with the app theme removes that
    // dark edge; "system" passes null to hand control back to the OS.
    this.syncNativeWindowTheme(newTheme);
  }

  /**
   * Align the native window appearance with the app theme (system → null,
   * back to OS-following). Tauri environment only; failures are silent —
   * a pure style sync must not break theme switching itself.
   */
  private syncNativeWindowTheme(theme: Theme): void {
    if (!isTauriEnvironment()) return;
    import("@tauri-apps/api/window")
      .then(({ getCurrentWindow }) =>
        getCurrentWindow().setTheme(theme === "system" ? null : theme),
      )
      .catch((error) => {
        console.error("Failed to sync native window theme:", error);
      });
  }

  setLanguage(lang: Language): void {
    this.state.language = lang;

    // Persist to localStorage (fast startup + cross-window sync).
    if (typeof localStorage !== "undefined") {
      const current = localStorage.getItem(LANGUAGE_KEY);
      if (current !== lang) {
        localStorage.setItem(LANGUAGE_KEY, lang);
      }
    }

    if (typeof document !== "undefined") {
      document.documentElement.lang = lang;
    }
  }

  /**
   * Passive language sync: updates in-memory state and document.lang only,
   * NEVER writing back to localStorage.
   *
   * Dedicated to handling `storage` events from other windows. The initiator
   * already wrote the new language to shared localStorage; if the follower
   * wrote back via `setLanguage`, it would trigger another round of
   * cross-window broadcast. With multiple windows (main / settings / 3
   * selection panels) full-page reloading on background-resume, that
   * write-back and the authoritative backfill overwrite each other — the root
   * cause of the zh/en flip-flop. Keeping the passive path one-way read-only
   * eliminates the ping-pong at the source.
   */
  syncLanguageFromExternal(lang: Language): void {
    this.state.language = lang;

    if (typeof document !== "undefined") {
      document.documentElement.lang = lang;
    }
  }

  /**
   * Remember the most recently opened agent session id (restored when
   * switching back to Agent mode) and persist it.
   */
  setLastAgentSessionId(id: string | null): void {
    this.state.lastAgentSessionId = id;

    if (typeof localStorage !== "undefined") {
      if (id) {
        localStorage.setItem(LAST_AGENT_SESSION_ID_KEY, id);
      } else {
        localStorage.removeItem(LAST_AGENT_SESSION_ID_KEY);
      }
    }
  }
}

export const uiState = new UIState();
