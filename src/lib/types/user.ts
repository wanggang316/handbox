export interface User {
  id: string;
  username: string;
  email: string;
  avatar?: string;
  isPro: boolean;
  createdAt: string;
  updatedAt: string;
}

export type AuthProvider = 'google' | 'github' | 'email';

export interface GoogleLoginRequest {
  /** Google OAuth authorization code. */
  code: string;
  redirectUri: string;
}

export interface AuthResponse {
  user: User;
  accessToken: string;
  refreshToken: string;
  /** Token lifetime in seconds. */
  expiresIn: number;
}

export interface RefreshTokenRequest {
  refreshToken: string;
}

export interface UpdateUserProfileRequest {
  username?: string;
  avatar?: string;
}

export interface UserState {
  /** Null when not logged in. */
  user: User | null;
  isLoggedIn: boolean;
  accessToken: string | null;
  isLoading: boolean;
}
