/**
 * HTTP client wrapper with JWT token injection and refresh handling
 */

import { tokenStorage } from '$lib/utils/storage';

// API base URL - can be configured via environment variable
const API_BASE_URL = '/api/v1';

/**
 * API Error response structure matching backend ErrorResponse
 */
export interface ApiError {
	error: string;
	code: string;
	details?: string;
}

/**
 * Custom error class for API errors
 */
export class ApiRequestError extends Error {
	public readonly status: number;
	public readonly code: string;
	public readonly details?: string;

	constructor(message: string, status: number, code: string, details?: string) {
		super(message);
		this.name = 'ApiRequestError';
		this.status = status;
		this.code = code;
		this.details = details;
	}
}

/**
 * Token pair returned from auth endpoints
 */
export interface AccessTokenResponse {
	accessToken: string;
	expiresAt: string;
}

/**
 * Get stored access token
 */
export function getAccessToken(): string | null {
	return tokenStorage.getAccessToken();
}

/**
 * Get stored refresh token
 */
export function setAccessToken(accessToken: string): void {
	tokenStorage.setAccessToken(accessToken);
}

/**
 * Clear stored tokens
 */
export function clearTokens(): void {
	tokenStorage.clearTokens();
}

/**
 * Check if user is authenticated (has tokens)
 */
export function isAuthenticated(): boolean {
	return tokenStorage.hasTokens();
}

// Flag to prevent multiple simultaneous refresh attempts
let isRefreshing = false;
let refreshPromise: Promise<boolean> | null = null;

/**
 * Attempt to refresh the access token using the refresh token
 * Returns true if refresh was successful, false otherwise
 */
async function refreshAccessToken(): Promise<boolean> {
	// If already refreshing, wait for that to complete
	if (isRefreshing && refreshPromise) {
		return refreshPromise;
	}

	isRefreshing = true;
	refreshPromise = (async () => {
		try {
			const response = await fetch(`${API_BASE_URL}/auth/refresh`, {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json'
				},
				credentials: 'same-origin'
			});

			if (!response.ok) {
				// Refresh failed - clear tokens
				clearTokens();
				return false;
			}

			const data: AccessTokenResponse = await response.json();
			setAccessToken(data.accessToken);
			return true;
		} catch {
			clearTokens();
			return false;
		} finally {
			isRefreshing = false;
			refreshPromise = null;
		}
	})();

	return refreshPromise;
}

/**
 * Request options for the API client
 */
export interface RequestOptions {
	method?: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';
	body?: unknown;
	headers?: Record<string, string>;
	skipAuth?: boolean;
	params?: Record<string, string | number | boolean | undefined>;
	signal?: AbortSignal;
}

/**
 * Build URL with query parameters
 */
function buildUrl(
	endpoint: string,
	params?: Record<string, string | number | boolean | undefined>
): string {
	const url = new URL(`${API_BASE_URL}${endpoint}`, window.location.origin);

	if (params) {
		Object.entries(params).forEach(([key, value]) => {
			if (value !== undefined && value !== '') {
				url.searchParams.append(key, String(value));
			}
		});
	}

	return url.toString();
}

/**
 * Parse response and handle errors
 */
async function parseResponse<T>(response: Response): Promise<T> {
	const contentType = response.headers.get('content-type');

	if (!response.ok) {
		let errorData: ApiError = {
			error: 'Unknown error',
			code: 'UNKNOWN_ERROR'
		};

		if (contentType?.includes('application/json')) {
			try {
				errorData = await response.json();
			} catch {
				// Use default error
			}
		}

		throw new ApiRequestError(errorData.error, response.status, errorData.code, errorData.details);
	}

	// Handle empty responses
	if (response.status === 204 || !contentType?.includes('application/json')) {
		return {} as T;
	}

	return response.json();
}

/**
 * Main API request function with automatic token injection and refresh
 */
export async function apiRequest<T>(endpoint: string, options: RequestOptions = {}): Promise<T> {
	const { method = 'GET', body, headers = {}, skipAuth = false, params, signal } = options;

	const url = buildUrl(endpoint, params);

	const requestHeaders: Record<string, string> = {
		...headers
	};

	// Add Content-Type for requests with body
	if (body && !requestHeaders['Content-Type']) {
		requestHeaders['Content-Type'] = 'application/json';
	}

	// Add Authorization header if authenticated and not skipping auth
	if (!skipAuth) {
		const accessToken = getAccessToken();
		if (accessToken) {
			requestHeaders['Authorization'] = `Bearer ${accessToken}`;
		}
	}

	const fetchOptions: RequestInit = {
		method,
		headers: requestHeaders,
		signal,
		credentials: 'same-origin'
	};

	if (body) {
		fetchOptions.body = JSON.stringify(body);
	}

	let response = await fetch(url, fetchOptions);

	// Handle 401 - attempt token refresh and retry
	if (response.status === 401 && !skipAuth) {
		const refreshed = await refreshAccessToken();

		if (refreshed) {
			// Retry with new token
			const newAccessToken = getAccessToken();
			if (newAccessToken) {
				requestHeaders['Authorization'] = `Bearer ${newAccessToken}`;
				fetchOptions.headers = requestHeaders;
				response = await fetch(url, fetchOptions);
			}
		}
	}

	return parseResponse<T>(response);
}

/**
 * Convenience methods for common HTTP methods
 */
export const api = {
	get: <T>(
		endpoint: string,
		params?: Record<string, string | number | boolean | undefined>,
		signal?: AbortSignal
	) => apiRequest<T>(endpoint, { method: 'GET', params, signal }),

	post: <T>(endpoint: string, body?: unknown) => apiRequest<T>(endpoint, { method: 'POST', body }),

	put: <T>(endpoint: string, body?: unknown) => apiRequest<T>(endpoint, { method: 'PUT', body }),

	patch: <T>(endpoint: string, body?: unknown) =>
		apiRequest<T>(endpoint, { method: 'PATCH', body }),

	delete: <T>(endpoint: string, params?: Record<string, string | number | boolean | undefined>) =>
		apiRequest<T>(endpoint, { method: 'DELETE', params })
};
