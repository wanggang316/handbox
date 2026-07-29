import { isTauri as isTauriCore, convertFileSrc } from "@tauri-apps/api/core";
import { openPath as openSystemResource } from "@tauri-apps/plugin-opener";

/**
 * Detect whether the code runs inside a Tauri app.
 *
 * Prefers the official `isTauri()` API, then falls back to the globals Tauri
 * injects: `__TAURI_INTERNALS__` (v2), `window.isTauri` (v2.0.0-beta.9+),
 * `__TAURI__` (v1, requires withGlobalTauri).
 */
export function isTauriEnvironment(): boolean {
  if (typeof window === "undefined") {
    return false;
  }

  try {
    return isTauriCore();
  } catch {
    // Fall through to manual checks.
  }

  if ("__TAURI_INTERNALS__" in window) {
    return true;
  }

  if ("isTauri" in window && (window as any).isTauri === true) {
    return true;
  }

  if ("__TAURI__" in window) {
    return true;
  }

  return false;
}

export function ensureTauriEnvironment(): void {
  if (!isTauriEnvironment()) {
    throw new Error("This function can only be called in a Tauri environment");
  }
}

/** Environment-detection details, for debugging. */
export function getTauriEnvironmentInfo() {
  if (typeof window === "undefined") {
    return {
      isTauri: false,
      usesOfficialApi: false,
      hasTauriInternals: false,
      hasTauriGlobal: false,
      hasIsTauriProperty: false,
      platform: "server",
    };
  }

  let usesOfficialApi = false;
  try {
    usesOfficialApi = isTauriCore();
  } catch {
    // API unavailable.
  }

  const hasTauriInternals = "__TAURI_INTERNALS__" in window;
  const hasTauriGlobal = "__TAURI__" in window;
  const hasIsTauriProperty =
    "isTauri" in window && (window as any).isTauri === true;

  return {
    isTauri:
      usesOfficialApi ||
      hasTauriInternals ||
      hasTauriGlobal ||
      hasIsTauriProperty,
    usesOfficialApi,
    hasTauriInternals,
    hasTauriGlobal,
    hasIsTauriProperty,
    platform: "browser",
  };
}

/**
 * Convert a local file path into a WebView-accessible URL.
 * data:/blob:/http(s) URLs pass through; paths go through convertFileSrc in Tauri.
 */
export function resolveLocalAssetPath(path?: string): string {
  if (!path) return "";
  const lower = path.toLowerCase();
  if (
    lower.startsWith("data:") ||
    lower.startsWith("blob:") ||
    lower.startsWith("http://") ||
    lower.startsWith("https://")
  ) {
    return path;
  }
  return isTauriEnvironment() ? convertFileSrc(path) : path;
}

export async function openPathInSystem(path: string): Promise<void> {
  if (!path) return;
  const targetUrl = path.startsWith("file://") ? path : `file://${path}`;
  const normalizedPath = path.replace(/^file:\/\//, "");

  if (!isTauriEnvironment()) {
    if (typeof window !== "undefined") {
      window.open(targetUrl, "_blank", "noopener,noreferrer");
    }
    return;
  }

  try {
    await openSystemResource(normalizedPath);
  } catch (error) {
    console.error("[openPathInSystem] Failed to open path:", error);
    if (typeof window !== "undefined") {
      window.open(targetUrl, "_blank", "noopener,noreferrer");
    }
  }
}
