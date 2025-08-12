/**
 * 验证工具函数
 */

/**
 * 验证 URL 格式
 */
export function isValidUrl(url: string): boolean {
  try {
    new URL(url);
    return true;
  } catch {
    return false;
  }
}

/**
 * 验证 API Key 格式（基础检查）
 */
export function isValidApiKey(apiKey: string): boolean {
  return apiKey.length >= 8 && !apiKey.includes(' ');
}

/**
 * 验证邮箱格式
 */
export function isValidEmail(email: string): boolean {
  const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  return emailRegex.test(email);
}

/**
 * 验证文件类型
 */
export function isValidImageFile(file: File): boolean {
  const allowedTypes = ['image/jpeg', 'image/png', 'image/webp', 'image/gif'];
  return allowedTypes.includes(file.type);
}

/**
 * 验证文件大小
 */
export function isValidFileSize(file: File, maxSizeMB = 10): boolean {
  const maxSizeBytes = maxSizeMB * 1024 * 1024;
  return file.size <= maxSizeBytes;
}

/**
 * 验证模型参数
 */
export function validateModelParameters(params: {
  temperature?: number;
  topP?: number;
  maxTokens?: number;
  contextLength?: number;
}): { valid: boolean; errors: string[] } {
  const errors: string[] = [];
  
  if (params.temperature !== undefined) {
    if (params.temperature < 0 || params.temperature > 2) {
      errors.push('Temperature 应该在 0-2 之间');
    }
  }
  
  if (params.topP !== undefined) {
    if (params.topP < 0 || params.topP > 1) {
      errors.push('Top-P 应该在 0-1 之间');
    }
  }
  
  if (params.maxTokens !== undefined) {
    if (params.maxTokens < 1 || params.maxTokens > 1000000) {
      errors.push('最大 Token 数应该在 1-1000000 之间');
    }
  }
  
  if (params.contextLength !== undefined) {
    if (params.contextLength < 1 || params.contextLength > 1000000) {
      errors.push('上下文长度应该在 1-1000000 之间');
    }
  }
  
  return {
    valid: errors.length === 0,
    errors
  };
}

/**
 * 验证 MCP 服务器配置
 */
export function validateMCPServer(server: {
  name: string;
  command: string;
  args: string[];
}): { valid: boolean; errors: string[] } {
  const errors: string[] = [];
  
  if (!server.name.trim()) {
    errors.push('服务器名称不能为空');
  }
  
  if (!server.command.trim()) {
    errors.push('命令不能为空');
  }
  
  if (!Array.isArray(server.args)) {
    errors.push('参数必须是数组');
  }
  
  return {
    valid: errors.length === 0,
    errors
  };
}

/**
 * 验证 JSON 格式
 */
export function isValidJSON(jsonString: string): boolean {
  try {
    JSON.parse(jsonString);
    return true;
  } catch {
    return false;
  }
}

/**
 * 验证搜索查询
 */
export function validateSearchQuery(query: string): { valid: boolean; error?: string } {
  if (!query.trim()) {
    return { valid: false, error: '搜索查询不能为空' };
  }
  
  if (query.length > 1000) {
    return { valid: false, error: '搜索查询过长（最大1000字符）' };
  }
  
  return { valid: true };
}

/**
 * 验证 UUID 格式
 */
export function isValidUUID(uuid: string): boolean {
  const uuidRegex = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
  return uuidRegex.test(uuid);
}