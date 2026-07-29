import { apiCall } from "./index";

/**
 * Loads an external image through the backend proxy to bypass WebView access
 * restrictions (HTTPS only). Returns a base64 data URL.
 */
export async function proxyImage(url: string): Promise<string> {
  try {
    const imageBytes = await apiCall<number[]>("image_proxy", { url });

    const uint8Array = new Uint8Array(imageBytes);

    const base64 = btoa(
      uint8Array.reduce((data, byte) => data + String.fromCharCode(byte), ""),
    );

    // Infer the MIME type from the URL extension.
    const extension = url.split(".").pop()?.toLowerCase();
    let mimeType = "image/jpeg";

    if (extension === "png") {
      mimeType = "image/png";
    } else if (extension === "gif") {
      mimeType = "image/gif";
    } else if (extension === "webp") {
      mimeType = "image/webp";
    } else if (extension === "svg") {
      mimeType = "image/svg+xml";
    }

    return `data:${mimeType};base64,${base64}`;
  } catch (error) {
    console.error("Failed to proxy image:", error);
    throw error;
  }
}

export function shouldProxyImage(url: string | undefined): boolean {
  if (!url) return false;

  const proxyDomains = ["googleusercontent.com", "googleapis.com"];

  return proxyDomains.some((domain) => url.includes(domain));
}
