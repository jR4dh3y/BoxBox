/**
 * HTTP client wrapper with JWT token injection and refresh handling
 */

import { tokenStorage } from '#lib/utils/storage.js';

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
 * Store an access token and reopen refresh for the authenticated session.
 */
export function setAccessToken(accessToken: string): void {
	tokenStorage.setAccessToken(accessToken);
	sessionEnded = false;
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

let refreshPromise: Promise<AccessTokenResponse> | null = null;
let logoutPromise: Promise<void> | null = null;
let logoutInProgress = false;
let sessionEnded = false;

export class AuthSessionClosedError extends Error {
	constructor() {
		super('The session is no longer available');
		this.name = 'AuthSessionClosedError';
	}
}

export type AuthRefreshResult = { kind: 'succeeded' } | { kind: 'failed'; error: unknown };

export class AuthRefreshEvent extends Event {
	static readonly eventName = 'boxbox-auth-refresh';

	constructor(readonly result: AuthRefreshResult) {
		super(AuthRefreshEvent.eventName);
	}
}

function isTerminalAuthFailure(error: unknown): error is ApiRequestError {
	return error instanceof ApiRequestError && error.status === 401;
}

// A parallel tab may have rotated the shared cookie. Retry once without ending the session.
async function requestSessionMutation<T>(endpoint: '/auth/refresh' | '/auth/logout'): Promise<T> {
	const request = () => apiRequest<T>(endpoint, { method: 'POST', skipAuth: true });
	try {
		return await request();
	} catch (error) {
		if (
			!(error instanceof ApiRequestError) ||
			error.status !== 409 ||
			error.code !== 'REFRESH_SUPERSEDED'
		)
			throw error;
		return request();
	}
}

/**
 * Refresh the session once for all same-tab callers.
 */
export async function refreshSession(): Promise<AccessTokenResponse> {
	if (sessionEnded || logoutInProgress || logoutPromise) {
		throw new AuthSessionClosedError();
	}
	if (refreshPromise) return refreshPromise;

	const operation = (async () => {
		try {
			const response = await requestSessionMutation<AccessTokenResponse>('/auth/refresh');
			setAccessToken(response.accessToken);
			if (typeof window !== 'undefined') {
				window.dispatchEvent(new AuthRefreshEvent({ kind: 'succeeded' }));
			}
			return response;
		} catch (error) {
			if (isTerminalAuthFailure(error)) {
				sessionEnded = true;
				clearTokens();
			}
			if (typeof window !== 'undefined') {
				window.dispatchEvent(new AuthRefreshEvent({ kind: 'failed', error }));
			}
			throw error;
		}
	})();
	refreshPromise = operation;

	try {
		return await operation;
	} finally {
		if (refreshPromise === operation) refreshPromise = null;
	}
}

/**
 * Log out only after any in-flight refresh finishes; do not refresh this request.
 */
export function logoutSession(): Promise<void> {
	if (logoutPromise) return logoutPromise;
	logoutInProgress = true;

	const operation = (async () => {
		try {
			const pendingRefresh = refreshPromise;
			if (pendingRefresh) {
				try {
					await pendingRefresh;
				} catch (error) {
					if (isTerminalAuthFailure(error)) throw error;
				}
			}

			try {
				await requestSessionMutation<unknown>('/auth/logout');
			} catch (error) {
				if (isTerminalAuthFailure(error)) {
					sessionEnded = true;
					clearTokens();
				}
				throw error;
			}

			sessionEnded = true;
			clearTokens();
		} finally {
			logoutInProgress = false;
		}
	})();
	logoutPromise = operation;

	return operation.finally(() => {
		if (logoutPromise === operation) logoutPromise = null;
	});
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
export function buildUrl(
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
export async function parseResponse<T>(response: Response): Promise<T> {
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

	// Every endpoint answers with JSON. Anything else (such as the SPA fallback page) is not a success.
	if (!contentType?.includes('application/json')) {
		throw new ApiRequestError(
			'Unexpected response from server',
			response.status,
			'INVALID_RESPONSE'
		);
	}

	return response.json();
}

/**
 * Main API request function with automatic token injection and refresh
 */
export function apiRequest<T>(endpoint: string, options?: RequestOptions): Promise<T>;
export async function apiRequest(endpoint: string, options: RequestOptions = {}): Promise<unknown> {
	const response = await apiFetch(endpoint, options);
	if (response.status === 204) return;
	return parseResponse<unknown>(response);
}

/** Authenticated transport shared by JSON and binary endpoints. */
export async function apiFetch(endpoint: string, options: RequestOptions = {}): Promise<Response> {
	const { method = 'GET', body, headers = {}, skipAuth = false, params, signal } = options;

	const url = buildUrl(endpoint, params);

	const requestHeaders: Record<string, string> = {
		...headers
	};

	// Blobs are sent as-is (binary uploads); everything else is JSON.
	const isBlobBody = body instanceof Blob;
	if (body && !requestHeaders['Content-Type']) {
		requestHeaders['Content-Type'] = isBlobBody
			? body.type || 'application/octet-stream'
			: 'application/json';
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
		fetchOptions.body = isBlobBody ? body : JSON.stringify(body);
	}

	let response = await fetch(url, fetchOptions);

	// A logout response is terminal for this request; never rotate after its 401.
	if (response.status === 401 && !skipAuth) {
		try {
			const refreshed = await refreshSession();
			requestHeaders['Authorization'] = `Bearer ${refreshed.accessToken}`;
			fetchOptions.headers = requestHeaders;
			response = await fetch(url, fetchOptions);
		} catch (error) {
			if (!(error instanceof AuthSessionClosedError)) throw error;
		}
	}

	return response;
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
