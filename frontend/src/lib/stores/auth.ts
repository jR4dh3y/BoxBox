/**
 * Auth store for managing authentication state
 */

import { writable, derived, get } from 'svelte/store';
import {
	login as apiLogin,
	logout as apiLogout,
	refresh as apiRefresh,
	isAuthenticated as checkAuth
} from '$lib/api/auth';
import { CONFIG } from '$lib/config';
import { settingsStore } from './settings';

/**
 * Auth state interface
 */
export interface AuthState {
	isAuthenticated: boolean;
	isDevelopment: boolean;
	isLoading: boolean;
	error: string | null;
	username: string | null;
}

/**
 * Initial auth state
 */
const initialState: AuthState = {
	isAuthenticated: false,
	isDevelopment: false,
	isLoading: false,
	error: null,
	username: null
};

/**
 * Create the auth store
 */
function createAuthStore() {
	const { subscribe, set, update } = writable<AuthState>(initialState);

	let refreshInterval: ReturnType<typeof setInterval> | null = null;

	/**
	 * Initialize auth state from stored tokens
	 */
	function initialize(): void {
		const isDevelopment =
			document.querySelector('meta[name="boxbox-dev-mode"]')?.getAttribute('content') === 'true';
		const isAuth = checkAuth();
		update((state) => ({
			...state,
			isAuthenticated: isDevelopment || isAuth,
			isDevelopment,
			username: isDevelopment ? 'dev' : state.username
		}));

		if (isDevelopment) {
			settingsStore.initialize();
		} else if (isAuth) {
			startTokenRefresh();
			// Load user settings after auth is confirmed
			settingsStore.initialize();
		}
	}

	/**
	 * Start automatic token refresh
	 */
	function startTokenRefresh(): void {
		stopTokenRefresh();
		refreshInterval = setInterval(async () => {
			try {
				await apiRefresh();
			} catch {
				stopTokenRefresh();
				set({
					...initialState,
					error: 'Session expired. Please log in again.'
				});
			}
		}, CONFIG.auth.tokenRefreshIntervalMs);
	}

	/**
	 * Stop automatic token refresh
	 */
	function stopTokenRefresh(): void {
		if (refreshInterval) {
			clearInterval(refreshInterval);
			refreshInterval = null;
		}
	}

	/**
	 * Login with username and password
	 */
	async function login(username: string, password: string): Promise<boolean> {
		update((state) => ({
			...state,
			isLoading: true,
			error: null
		}));

		try {
			await apiLogin(username, password);
			update((state) => ({
				...state,
				isAuthenticated: true,
				isLoading: false,
				username
			}));
			startTokenRefresh();
			// Load user settings after successful login
			settingsStore.initialize();
			return true;
		} catch (err) {
			const message = err instanceof Error ? err.message : 'Login failed';
			update((state) => ({
				...state,
				isLoading: false,
				error: message
			}));
			return false;
		}
	}

	/**
	 * Logout and clear tokens
	 */
	async function logout(): Promise<void> {
		stopTokenRefresh();
		try {
			await apiLogout();
		} catch {
			// Ignore logout errors
		}
		set(initialState);
	}

	/**
	 * Manually refresh the access token
	 */
	async function refresh(): Promise<boolean> {
		try {
			await apiRefresh();
			return true;
		} catch {
			stopTokenRefresh();
			set({
				...initialState,
				error: 'Session expired. Please log in again.'
			});
			return false;
		}
	}

	/**
	 * Clear any error message
	 */
	function clearError(): void {
		update((state) => ({
			...state,
			error: null
		}));
	}

	/**
	 * Check if currently authenticated
	 */
	function isAuthenticated(): boolean {
		return get({ subscribe }).isAuthenticated;
	}

	return {
		subscribe,
		initialize,
		login,
		logout,
		refresh,
		clearError,
		isAuthenticated
	};
}

/**
 * Auth store singleton
 */
export const authStore = createAuthStore();

/**
 * Derived store for just the authentication status
 */
export const isAuthenticated = derived(authStore, ($auth) => $auth.isAuthenticated);

/** Whether the server is running with --dev authentication bypass. */
export const isDevelopment = derived(authStore, ($auth) => $auth.isDevelopment);

/**
 * Derived store for loading state
 */
export const isAuthLoading = derived(authStore, ($auth) => $auth.isLoading);

/**
 * Derived store for auth error
 */
export const authError = derived(authStore, ($auth) => $auth.error);
