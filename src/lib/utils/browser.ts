/**
 * 浏览器相关工具函数
 */

import { browser } from "$app/environment";
import { openUrl } from "@tauri-apps/plugin-opener";

/**
 * 复制文本到剪贴板，自动降级兼容旧环境
 */
export async function copyToClipboard(content: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(content);
  } catch {
    const textarea = document.createElement("textarea");
    textarea.value = content;
    textarea.setAttribute("readonly", "");
    textarea.style.position = "absolute";
    textarea.style.left = "-9999px";
    document.body.appendChild(textarea);
    textarea.select();
    try {
      document.execCommand("copy");
    } catch (fallbackError) {
      console.error("Failed to copy to clipboard", fallbackError);
    }
    document.body.removeChild(textarea);
  }
}

/**
 * 在系统默认浏览器中打开URL
 * @param url 要打开的URL
 */
export async function openInBrowser(url: string): Promise<void> {
  try {
    await openUrl(url);
    return;
  } catch (error) {
    if (browser) {
      window.open(url, "_blank", "noopener,noreferrer");
      return;
    }

    console.error("Failed to open URL in browser:", error);
    throw error;
  }
}
