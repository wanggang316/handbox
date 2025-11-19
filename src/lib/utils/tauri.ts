/**
 * Tauri 环境检测工具
 *
 * 提供统一的 Tauri 环境检测方法，用于判断代码是否运行在 Tauri 应用中
 */

import { isTauri as isTauriCore } from "@tauri-apps/api/core";

/**
 * 检测当前是否运行在 Tauri 环境中
 *
 * 使用 Tauri 2.0 官方推荐的方式进行检测：
 * 1. 优先使用 `@tauri-apps/api/core` 的 `isTauri()` 函数（官方 API，最可靠）
 * 2. 降级检查 `window.__TAURI_INTERNALS__`（内部标识，始终存在于 Tauri 2.0+）
 * 3. 降级检查 `window.isTauri`（v2.0.0-beta.9+ 添加的全局属性）
 * 4. 降级检查 `window.__TAURI__`（v1 兼容，需要在 tauri.conf.json 中启用 withGlobalTauri）
 *
 * @returns {boolean} 如果运行在 Tauri 环境中返回 true，否则返回 false
 *
 * @see https://v2.tauri.app/reference/javascript/api/namespacecore/#istauri
 * @see https://github.com/tauri-apps/tauri/discussions/6119
 *
 * @example
 * ```typescript
 * import { isTauriEnvironment } from '$lib/utils/tauri';
 *
 * if (isTauriEnvironment()) {
 *   // 使用 Tauri API
 *   const { invoke } = await import('@tauri-apps/api/core');
 *   await invoke('some_command');
 * } else {
 *   // 使用 Web API 或显示提示
 *   console.log('请在 Tauri 应用中运行');
 * }
 * ```
 */
export function isTauriEnvironment(): boolean {
  // 服务端渲染环境
  if (typeof window === "undefined") {
    return false;
  }

  // 方法 1: 使用官方 API（推荐，最可靠）
  try {
    return isTauriCore();
  } catch {
    // 如果 API 调用失败，降级到手动检查
  }

  // 方法 2: 检查 __TAURI_INTERNALS__（v2 内部标识）
  if ("__TAURI_INTERNALS__" in window) {
    return true;
  }

  // 方法 3: 检查 window.isTauri（v2.0.0-beta.9+ 添加）
  if ("isTauri" in window && (window as any).isTauri === true) {
    return true;
  }

  // 方法 4: 检查 __TAURI__（v1 兼容，需要配置启用）
  if ("__TAURI__" in window) {
    return true;
  }

  return false;
}

/**
 * 确保代码运行在 Tauri 环境中，否则抛出错误
 *
 * @throws {Error} 如果不在 Tauri 环境中
 *
 * @example
 * ```typescript
 * import { ensureTauriEnvironment } from '$lib/utils/tauri';
 *
 * function someFunction() {
 *   ensureTauriEnvironment();
 *   // 下面的代码可以安全地使用 Tauri API
 * }
 * ```
 */
export function ensureTauriEnvironment(): void {
  if (!isTauriEnvironment()) {
    throw new Error("This function can only be called in a Tauri environment");
  }
}

/**
 * 获取 Tauri 环境的详细信息（用于调试）
 *
 * @returns {object} 包含环境检测详情的对象
 *
 * @example
 * ```typescript
 * import { getTauriEnvironmentInfo } from '$lib/utils/tauri';
 *
 * console.log(getTauriEnvironmentInfo());
 * // {
 * //   isTauri: true,
 * //   hasTauriInternals: true,
 * //   hasTauriGlobal: false,
 * //   hasCustomFlag: false
 * // }
 * ```
 */
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
    // API 不可用
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
