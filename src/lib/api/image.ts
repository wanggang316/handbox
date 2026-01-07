/**
 * 图片相关 API
 *
 * 提供图片代理加载功能
 */

import { apiCall } from './index';

/**
 * 代理加载图片
 *
 * 通过后端代理方式加载外部图片，避免 WebView 的访问限制
 *
 * @param url - 图片 URL（仅支持 HTTPS）
 * @returns 图片的 Base64 数据 URL (data:image/...)
 *
 * 后端接口约定:
 * - 命令: image_proxy
 * - 参数: { url: string }
 * - 返回: number[] (图片二进制数据)
 */
export async function proxyImage(url: string): Promise<string> {
  try {
    // 调用后端代理接口获取图片数据
    const imageBytes = await apiCall<number[]>('image_proxy', { url });

    // 将字节数组转换为 Uint8Array
    const uint8Array = new Uint8Array(imageBytes);

    // 转换为 Base64
    const base64 = btoa(
      uint8Array.reduce((data, byte) => data + String.fromCharCode(byte), '')
    );

    // 根据 URL 判断图片类型
    const extension = url.split('.').pop()?.toLowerCase();
    let mimeType = 'image/jpeg'; // 默认 JPEG

    if (extension === 'png') {
      mimeType = 'image/png';
    } else if (extension === 'gif') {
      mimeType = 'image/gif';
    } else if (extension === 'webp') {
      mimeType = 'image/webp';
    } else if (extension === 'svg') {
      mimeType = 'image/svg+xml';
    }

    // 返回 data URL
    return `data:${mimeType};base64,${base64}`;
  } catch (error) {
    console.error('Failed to proxy image:', error);
    throw error;
  }
}

/**
 * 检查 URL 是否需要代理加载
 *
 * @param url - 图片 URL
 * @returns 是否需要代理
 */
export function shouldProxyImage(url: string | undefined): boolean {
  if (!url) return false;

  // 需要代理的域名列表
  const proxyDomains = [
    'googleusercontent.com',
    'googleapis.com',
    // 可以添加更多需要代理的域名
  ];

  return proxyDomains.some(domain => url.includes(domain));
}
