export function isValidUrl(url: string): boolean {
  try {
    new URL(url);
    return true;
  } catch {
    return false;
  }
}

/** Basic shape check only — not provider-specific key validation. */
export function isValidApiKey(apiKey: string): boolean {
  return apiKey.length >= 8 && !apiKey.includes(' ');
}

export function isValidEmail(email: string): boolean {
  const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  return emailRegex.test(email);
}

export function isValidImageFile(file: File): boolean {
  const allowedTypes = ['image/jpeg', 'image/png', 'image/webp', 'image/gif'];
  return allowedTypes.includes(file.type);
}

export function isValidFileSize(file: File, maxSizeMB = 10): boolean {
  const maxSizeBytes = maxSizeMB * 1024 * 1024;
  return file.size <= maxSizeBytes;
}

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

export function isValidJSON(jsonString: string): boolean {
  try {
    JSON.parse(jsonString);
    return true;
  } catch {
    return false;
  }
}

export function validateSearchQuery(query: string): { valid: boolean; error?: string } {
  if (!query.trim()) {
    return { valid: false, error: '搜索查询不能为空' };
  }
  
  if (query.length > 1000) {
    return { valid: false, error: '搜索查询过长（最大1000字符）' };
  }
  
  return { valid: true };
}

export function isValidUUID(uuid: string): boolean {
  const uuidRegex = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
  return uuidRegex.test(uuid);
}