/**
 * 格式化工具函数
 */

/**
 * 格式化文件大小
 */
export function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

/**
 * 格式化数字（添加千分位分隔符）
 */
export function formatNumber(num: number): string {
  return num.toLocaleString();
}

/**
 * 格式化 Token 数量
 */
export function formatTokens(tokens: number): string {
  if (tokens < 1000) return tokens.toString();
  if (tokens < 1000000) return `${(tokens / 1000).toFixed(1)}K`;
  return `${(tokens / 1000000).toFixed(1)}M`;
}

/**
 * 格式化时长（毫秒）
 */
export function formatDuration(milliseconds: number): string {
  if (milliseconds < 1000) return `${milliseconds}ms`;
  if (milliseconds < 60000) return `${(milliseconds / 1000).toFixed(1)}s`;
  
  const minutes = Math.floor(milliseconds / 60000);
  const seconds = Math.floor((milliseconds % 60000) / 1000);
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}

/**
 * 格式化速度（字符/秒）
 */
export function formatSpeed(charactersPerSecond: number): string {
  if (charactersPerSecond < 10) return `${charactersPerSecond.toFixed(1)} chars/s`;
  return `${Math.round(charactersPerSecond)} chars/s`;
}

/**
 * 格式化百分比
 */
export function formatPercentage(value: number, total: number): string {
  if (total === 0) return '0%';
  const percentage = (value / total) * 100;
  return `${percentage.toFixed(1)}%`;
}

/**
 * 截断文本
 */
export function truncateText(text: string, maxLength: number, suffix = '...'): string {
  if (text.length <= maxLength) return text;
  return text.slice(0, maxLength - suffix.length) + suffix;
}

/**
 * 格式化模型名称（去除供应商前缀）
 */
export function formatModelName(modelId: string): string {
  // 移除常见的供应商前缀
  const prefixes = ['openai/', 'anthropic/', 'google/', 'deepseek/'];
  
  for (const prefix of prefixes) {
    if (modelId.startsWith(prefix)) {
      return modelId.slice(prefix.length);
    }
  }
  
  return modelId;
}

/**
 * 格式化错误消息
 */
export function formatErrorMessage(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === 'object' && 'message' in error) {
    return String(error.message);
  }
  return '未知错误';
}