/**
 * Auth API module for authentication operations
 * Requirements: 7.1
 */

import {
	apiRequest,
	setTokens,
	clearTokens,
	getRefreshToken,
	isAuthenticated as checkAuth
} from './client';

/**
 * Login request
 */
export interface LoginRequest {
	username: string;
	password: string;
}

/**
 * Login response
 */
export interface LoginResponse {
	accessToken: string;
	refreshToken: string;
	expiresAt: string;
}

/**
 * Logout request
 */
interface LogoutRequest {
	refreshToken: string;
}

/**
 * Success message response
 */
interface MessageResponse {
	message: string;
}

/**
 * Login with username and password
 * POST /api/v1/auth/login
 */
export async function login(username: string, password: string): Promise<LoginResponse> {
	const body: LoginRequest = { username, password };

	const response = await apiRequest<LoginResponse>('/auth/login', {
		method: 'POST',
		body,
		skipAuth: true
	});

	// Store tokens on successful login
	setTokens(response.accessToken, response.refreshToken);

	return response;
}

/**
 * Refresh access token using refresh token
 * POST /api/v1/auth/refresh
 */
export async function refresh(): Promise<LoginResponse> {
	const refreshToken = getRefreshToken();

	if (!refreshToken) {
		throw new Error('No refresh token available');
	}

	const response = await apiRequest<LoginResponse>('/auth/refresh', {
		method: 'POST',
		body: { refreshToken },
		skipAuth: true
	});

	// Store new tokens
	setTokens(response.accessToken, response.refreshToken);

	return response;
}

/**
 * Logout and invalidate refresh token
 * POST /api/v1/auth/logout
 */
export async function logout(): Promise<void> {
	const refreshToken = getRefreshToken();

	if (refreshToken) {
		try {
			const body: LogoutRequest = { refreshToken };
			await apiRequest<MessageResponse>('/auth/logout', {
				method: 'POST',
				body,
				skipAuth: true
			});
		} catch {
			// Ignore errors during logout - we'll clear tokens anyway
		}
	}

	// Always clear tokens locally
	clearTokens();
}

/**
 * Check if user is currently authenticated
 */
export function isAuthenticated(): boolean {
	return checkAuth();
}

// Re-export token utilities for convenience
export { getAccessToken, getRefreshToken, clearTokens } from './client';
