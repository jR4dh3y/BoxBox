/**
 * Auth store using Svelte 5 runes.
 */

import {
	login as apiLogin,
	logout as apiLogout,
	refresh as apiRefresh,
	isAuthenticated as checkAuth
} from '$lib/api/auth';
import { CONFIG } from '$lib/config';
import { settingsStore } from './settings.svelte';

export interface AuthState {
	isAuthenticated: boolean;
	isLoading: boolean;
	error: string | null;
	username: string | null;
}

const initialState: AuthState = {
	isAuthenticated: false,
	isLoading: false,
	error: null,
	username: null
};

class AuthStore {
	private state = $state<AuthState>({ ...initialState });
	private refreshInterval: ReturnType<typeof setInterval> | null = null;

	get isAuthenticated(): boolean {
		return this.state.isAuthenticated;
	}

	get isLoading(): boolean {
		return this.state.isLoading;
	}

	get error(): string | null {
		return this.state.error;
	}

	get username(): string | null {
		return this.state.username;
	}

	initialize(): void {
		const isAuth = checkAuth();
		this.state.isAuthenticated = isAuth;

		if (isAuth) {
			this.startTokenRefresh();
			void settingsStore.initialize();
		}
	}

	private startTokenRefresh(): void {
		this.stopTokenRefresh();
		this.refreshInterval = setInterval(async () => {
			try {
				await apiRefresh();
			} catch {
				this.stopTokenRefresh();
				this.state = {
					...initialState,
					error: 'Session expired. Please log in again.'
				};
			}
		}, CONFIG.auth.tokenRefreshIntervalMs);
	}

	private stopTokenRefresh(): void {
		if (this.refreshInterval) {
			clearInterval(this.refreshInterval);
			this.refreshInterval = null;
		}
	}

	async login(username: string, password: string): Promise<boolean> {
		this.state.isLoading = true;
		this.state.error = null;

		try {
			await apiLogin(username, password);
			this.state.isAuthenticated = true;
			this.state.isLoading = false;
			this.state.username = username;
			this.startTokenRefresh();
			void settingsStore.initialize();
			return true;
		} catch (err) {
			this.state.isLoading = false;
			this.state.error = err instanceof Error ? err.message : 'Login failed';
			return false;
		}
	}

	async logout(): Promise<void> {
		this.stopTokenRefresh();
		try {
			await apiLogout();
		} catch {
			// Ignore logout errors.
		}
		this.state = { ...initialState };
	}

	async refresh(): Promise<boolean> {
		try {
			await apiRefresh();
			return true;
		} catch {
			this.stopTokenRefresh();
			this.state = {
				...initialState,
				error: 'Session expired. Please log in again.'
			};
			return false;
		}
	}

	clearError(): void {
		this.state.error = null;
	}
}

export const authStore = new AuthStore();
