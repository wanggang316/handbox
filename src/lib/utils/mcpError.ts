import type { McpErrorType } from '$lib/types';

export function getErrorTypeDisplayName(errorType: McpErrorType): string {
  switch (errorType) {
    case 'connection_error':
      return '连接错误';
    case 'authentication_error':
      return '认证错误';
    case 'timeout_error':
      return '超时';
    case 'configuration_error':
      return '配置错误';
    case 'protocol_error':
      return '协议错误';
    case 'unknown_error':
      return '未知错误';
    default:
      return '未知错误';
  }
}
