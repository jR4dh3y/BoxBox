/**
 * Auth API module for authentication operations
 */

import { apiRequest, setAccessToken, clearTokens, isAuthenticated as checkAuth } from './client';
import { earlyStart, type EarlyStartSource } from './earlyStart';

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
	expiresAt: string;
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
	setAccessToken(response.accessToken);

	return response;
}

/**
 * Refresh access token using refresh token
 * POST /api/v1/auth/refresh
 */
export async function refresh(): Promise<LoginResponse> {
	const response = await apiRequest<LoginResponse>('/auth/refresh', {
		method: 'POST',
		skipAuth: true
	});

	setAccessToken(response.accessToken);

	return response;
}

/**
 * Restore the session when the page loads. Uses the refresh the page shell already started, if it
 * did, and asks the server otherwise. Never both: each refresh rotates the refresh token.
 */
export async function restoreSession(
	source: EarlyStartSource = earlyStart
): Promise<LoginResponse> {
	const early = await source.takeSession();
	if (!early) return refresh();
	setAccessToken(early.accessToken);
	return early;
}

/**
 * Logout and invalidate refresh token
 * POST /api/v1/auth/logout
 */
export async function logout(): Promise<void> {
	try {
		await apiRequest<MessageResponse>('/auth/logout', {
			method: 'POST'
		});
	} catch {
		// Ignore errors during logout - we'll clear the in-memory token anyway.
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
export { getAccessToken, clearTokens } from './client';
