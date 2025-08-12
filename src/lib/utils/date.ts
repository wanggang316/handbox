/**
 * 日期时间工具函数
 */

/**
 * 格式化时间戳为相对时间
 */
export function formatRelativeTime(timestamp: number, locale = 'zh-CN'): string {
  const now = Date.now();
  const diff = now - timestamp;
  
  const seconds = Math.floor(diff / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  const weeks = Math.floor(days / 7);
  const months = Math.floor(days / 30);
  const years = Math.floor(days / 365);
  
  if (locale === 'zh-CN') {
    if (seconds < 60) return '刚刚';
    if (minutes < 60) return `${minutes}分钟前`;
    if (hours < 24) return `${hours}小时前`;
    if (days < 7) return `${days}天前`;
    if (weeks < 4) return `${weeks}周前`;
    if (months < 12) return `${months}个月前`;
    return `${years}年前`;
  } else {
    if (seconds < 60) return 'just now';
    if (minutes < 60) return `${minutes}m ago`;
    if (hours < 24) return `${hours}h ago`;
    if (days < 7) return `${days}d ago`;
    if (weeks < 4) return `${weeks}w ago`;
    if (months < 12) return `${months}mo ago`;
    return `${years}y ago`;
  }
}

/**
 * 格式化时间戳为日期时间字符串
 */
export function formatDateTime(
  timestamp: number, 
  options: Intl.DateTimeFormatOptions = {},
  locale = 'zh-CN'
): string {
  const defaultOptions: Intl.DateTimeFormatOptions = {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    ...options
  };
  
  return new Date(timestamp).toLocaleString(locale, defaultOptions);
}

/**
 * 格式化时间戳为日期字符串
 */
export function formatDate(timestamp: number, locale = 'zh-CN'): string {
  return new Date(timestamp).toLocaleDateString(locale, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit'
  });
}

/**
 * 格式化时间戳为时间字符串
 */
export function formatTime(timestamp: number, locale = 'zh-CN'): string {
  return new Date(timestamp).toLocaleTimeString(locale, {
    hour: '2-digit',
    minute: '2-digit'
  });
}

/**
 * 判断是否为今天
 */
export function isToday(timestamp: number): boolean {
  const today = new Date();
  const date = new Date(timestamp);
  
  return date.getDate() === today.getDate() &&
         date.getMonth() === today.getMonth() &&
         date.getFullYear() === today.getFullYear();
}

/**
 * 判断是否为昨天
 */
export function isYesterday(timestamp: number): boolean {
  const yesterday = new Date();
  yesterday.setDate(yesterday.getDate() - 1);
  const date = new Date(timestamp);
  
  return date.getDate() === yesterday.getDate() &&
         date.getMonth() === yesterday.getMonth() &&
         date.getFullYear() === yesterday.getFullYear();
}

/**
 * 判断是否为本周
 */
export function isThisWeek(timestamp: number): boolean {
  const now = new Date();
  const date = new Date(timestamp);
  
  const startOfWeek = new Date(now);
  startOfWeek.setDate(now.getDate() - now.getDay());
  startOfWeek.setHours(0, 0, 0, 0);
  
  const endOfWeek = new Date(startOfWeek);
  endOfWeek.setDate(startOfWeek.getDate() + 6);
  endOfWeek.setHours(23, 59, 59, 999);
  
  return date >= startOfWeek && date <= endOfWeek;
}

/**
 * 获取友好的时间描述
 */
export function getFriendlyTimeLabel(timestamp: number, locale = 'zh-CN'): string {
  if (isToday(timestamp)) {
    return locale === 'zh-CN' ? '今天' : 'Today';
  }
  
  if (isYesterday(timestamp)) {
    return locale === 'zh-CN' ? '昨天' : 'Yesterday';
  }
  
  if (isThisWeek(timestamp)) {
    const dayNames = locale === 'zh-CN' 
      ? ['周日', '周一', '周二', '周三', '周四', '周五', '周六']
      : ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
    
    return dayNames[new Date(timestamp).getDay()];
  }
  
  return formatDate(timestamp, locale);
}

/**
 * 计算时间差（毫秒）
 */
export function getTimeDiff(start: number, end: number): number {
  return Math.abs(end - start);
}

/**
 * 获取当前时间戳
 */
export function getCurrentTimestamp(): number {
  return Date.now();
}