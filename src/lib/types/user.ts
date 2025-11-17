/**
 * 用户相关类型定义
 */

/**
 * 用户信息
 */
export interface User {
  /** 用户唯一标识 */
  id: string;
  /** 用户名 */
  username: string;
  /** 邮箱 */
  email: string;
  /** 头像 URL */
  avatar?: string;
  /** 是否为 Pro 用户 */
  isPro: boolean;
  /** 创建时间 */
  createdAt: string;
  /** 更新时间 */
  updatedAt: string;
}

/**
 * 认证提供商类型
 */
export type AuthProvider = 'google' | 'github' | 'email';

/**
 * Google 登录请求参数
 */
export interface GoogleLoginRequest {
  /** Google OAuth 授权码 */
  code: string;
  /** 重定向 URI */
  redirectUri: string;
}

/**
 * 认证响应
 */
export interface AuthResponse {
  /** 用户信息 */
  user: User;
  /** 访问令牌 */
  accessToken: string;
  /** 刷新令牌 */
  refreshToken: string;
  /** 令牌过期时间（秒） */
  expiresIn: number;
}

/**
 * 刷新令牌请求
 */
export interface RefreshTokenRequest {
  /** 刷新令牌 */
  refreshToken: string;
}

/**
 * 更新用户资料请求
 */
export interface UpdateUserProfileRequest {
  /** 用户名（可选） */
  username?: string;
  /** 头像 URL（可选） */
  avatar?: string;
}

/**
 * 用户状态
 */
export interface UserState {
  /** 当前用户（null 表示未登录） */
  user: User | null;
  /** 是否已登录 */
  isLoggedIn: boolean;
  /** 访问令牌 */
  accessToken: string | null;
  /** 是否正在加载 */
  isLoading: boolean;
}
