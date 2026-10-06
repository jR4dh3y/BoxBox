/**
 * Sign-in state. The access token lives in memory; the refresh token is an HttpOnly cookie.
 */

import {
	login as apiLogin,
	logout as apiLogout,
	refresh as apiRefresh,
	isAuthenticated as hasAccessToken
} from '$lib/api/auth';
import { CONFIG } from '$lib/config';
import { settingsStore } from './settings.svelte';
import { resetShareWallpaperLoginSync } from '$lib/utils/wallpaperSync';

class AuthStore {
	isAuthenticated = $state(false);
	/** True when the server runs with the --dev authentication bypass. */
	isDevelopment = $state(false);
	isLoading = $state(false);
	error = $state<string | null>(null);

	private refreshInterval: ReturnType<typeof setInterval> | null = null;

	/** Restores the session from the refresh-token cookie, if there is one. */
	async initialize(): Promise<void> {
		const isDevelopment =
			document.querySelector('meta[name="boxbox-dev-mode"]')?.getAttribute('content') === 'true';
		let isAuthenticated = hasAccessToken();
		if (!isDevelopment && !isAuthenticated) {
			try {
				await apiRefresh();
				isAuthenticated = true;
			} catch {
				isAuthenticated = false;
			}
		}

		this.isDevelopment = isDevelopment;
		this.isAuthenticated = isDevelopment || isAuthenticated;

		if (!isDevelopment && isAuthenticated) this.startTokenRefresh();
		if (this.isAuthenticated) void settingsStore.initialize();
	}

	async login(username: string, password: string): Promise<boolean> {
		this.isLoading = true;
		this.error = null;

		try {
			await apiLogin(username, password);
			this.isAuthenticated = true;
			this.isLoading = false;
			this.startTokenRefresh();
			void settingsStore.initialize();
			return true;
		} catch (err) {
			this.error = err instanceof Error ? err.message : 'Login failed';
			this.isLoading = false;
			return false;
		}
	}

	async logout(): Promise<void> {
		this.stopTokenRefresh();
		try {
			await apiLogout();
		} catch {
			// The local session is cleared either way.
		}
		this.reset();
	}

	clearError(): void {
		this.error = null;
	}

	private reset(error: string | null = null): void {
		resetShareWallpaperLoginSync();
		this.isAuthenticated = false;
		this.isDevelopment = false;
		this.isLoading = false;
		this.error = error;
	}

	private startTokenRefresh(): void {
		this.stopTokenRefresh();
		this.refreshInterval = setInterval(async () => {
			try {
				await apiRefresh();
			} catch {
				this.stopTokenRefresh();
				this.reset('Session expired. Please log in again.');
			}
		}, CONFIG.auth.tokenRefreshIntervalMs);
	}

	private stopTokenRefresh(): void {
		if (this.refreshInterval) {
			clearInterval(this.refreshInterval);
			this.refreshInterval = null;
		}
	}
}

export const authStore = new AuthStore();
