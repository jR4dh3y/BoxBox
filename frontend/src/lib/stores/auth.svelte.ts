/**
 * Sign-in state. The access token lives in memory; the refresh token is an HttpOnly cookie.
 */

import {
	login as apiLogin,
	logout as apiLogout,
	refresh as apiRefresh,
	restoreSession,
	isAuthenticated as hasAccessToken
} from '#lib/api/auth.js';
import { ApiRequestError, AuthSessionClosedError } from '#lib/api/client.js';
import { browseRequests } from '#lib/browse/requests.svelte.js';
import { CONFIG } from '#lib/config.js';
import { settingsStore } from './settings.svelte';
import { resetShareWallpaperLoginSync } from '#lib/utils/wallpaperSession.js';

type AuthAlert =
	| { kind: 'transient'; source: 'restore' | 'refresh' | 'logout'; message: string }
	| { kind: 'terminal'; message: string };

class AuthStore {
	isAuthenticated = $state(false);
	/** True when the server runs with the --dev authentication bypass. */
	isDevelopment = $state(false);
	isLoading = $state(false);
	isRestoringSession = $state(false);
	error = $state<string | null>(null);
	authAlert = $state<AuthAlert | null>(null);

	private refreshInterval: ReturnType<typeof setInterval> | null = null;

	/** Restores the session from the refresh-token cookie, if there is one. */
	async initialize(): Promise<void> {
		if (this.isRestoringSession || this.isLoading) return;
		this.isRestoringSession = true;
		try {
			const isDevelopment =
				document.querySelector('meta[name="boxbox-dev-mode"]')?.getAttribute('content') === 'true';
			let isAuthenticated = hasAccessToken();

			if (!isDevelopment && !isAuthenticated) {
				try {
					await restoreSession();
					isAuthenticated = true;
					this.clearTransientAuthAlert();
				} catch (error) {
					isAuthenticated = false;
					this.handleRefreshFailure(error, 'restore');
				}
			}

			this.isDevelopment = isDevelopment;
			if (isDevelopment || isAuthenticated) {
				if (document.querySelector('meta[name="boxbox-desktop"]')) await settingsStore.initialize();
				else void settingsStore.initialize();
			}
			if (isDevelopment || isAuthenticated) browseRequests.startSession();
			this.isAuthenticated = isDevelopment || isAuthenticated;

			if (!isDevelopment && isAuthenticated) this.startTokenRefresh();
		} finally {
			this.isRestoringSession = false;
		}
	}

	async login(username: string, password: string): Promise<boolean> {
		if (this.isLoading || this.isRestoringSession) return false;
		this.isLoading = true;
		this.error = null;

		try {
			await apiLogin(username, password);
			browseRequests.reset();
			browseRequests.startSession();
			void settingsStore.initialize();
			this.isAuthenticated = true;
			this.isLoading = false;
			this.authAlert = null;
			this.startTokenRefresh();
			return true;
		} catch (err) {
			this.error = err instanceof Error ? err.message : 'Login failed';
			this.isLoading = false;
			return false;
		}
	}

	async logout(): Promise<boolean> {
		this.stopTokenRefresh();

		try {
			await apiLogout();
			this.reset();
			this.authAlert = null;
			return true;
		} catch (error) {
			if (isTerminalAuthFailure(error)) {
				this.endSession(error);
				return true;
			}

			if (this.isAuthenticated) {
				if (!this.isDevelopment) this.startTokenRefresh();
				this.authAlert = {
					kind: 'transient',
					source: 'logout',
					message: 'Unable to log out. Your session is still active. Please try again.'
				};
			} else {
				this.authAlert = {
					kind: 'transient',
					source: 'logout',
					message: 'Unable to log out. Please try again.'
				};
			}
			return false;
		}
	}

	clearError(): void {
		this.error = null;
	}

	clearAuthAlert(): void {
		this.authAlert = null;
	}

	handleRefreshFailure(error: unknown, action: 'restore' | 'refresh'): void {
		if (error instanceof AuthSessionClosedError) return;
		const isRestoreAttempt =
			action === 'restore' || this.isRestoringSession || !this.isAuthenticated;
		if (isTerminalAuthFailure(error)) {
			if (isRestoreAttempt && error.code !== 'SESSION_RECOVERY_REQUIRED') {
				this.clearTransientAuthAlert();
				return;
			}
			if (!this.isAuthenticated && this.authAlert?.kind === 'terminal') return;
			this.endSession(error);
			return;
		}

		this.authAlert = {
			kind: 'transient',
			source: isRestoreAttempt ? 'restore' : 'refresh',
			message: isRestoreAttempt
				? 'Unable to restore your session right now. Please try again.'
				: "Unable to refresh your session right now. Your session is still active; we'll retry."
		};
	}

	handleRefreshSuccess(): void {
		this.clearTransientAuthAlert();
	}

	private endSession(error: ApiRequestError): void {
		this.stopTokenRefresh();
		this.reset();
		this.authAlert = {
			kind: 'terminal',
			message:
				error.code === 'SESSION_RECOVERY_REQUIRED'
					? 'Your session could not be safely restored. Please sign in again.'
					: 'Session expired. Please log in again.'
		};
	}

	private clearTransientAuthAlert(): void {
		if (this.authAlert?.kind === 'transient' && this.authAlert.source !== 'logout') {
			this.authAlert = null;
		}
	}

	private reset(): void {
		resetShareWallpaperLoginSync();
		browseRequests.reset();
		settingsStore.clearSession();
		this.isAuthenticated = false;
		this.isDevelopment = false;
		this.isLoading = false;
		this.error = null;
	}

	private startTokenRefresh(): void {
		this.stopTokenRefresh();
		this.refreshInterval = setInterval(async () => {
			try {
				await apiRefresh();
				this.clearTransientAuthAlert();
			} catch (error) {
				this.handleRefreshFailure(error, 'refresh');
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

function isTerminalAuthFailure(error: unknown): error is ApiRequestError {
	return error instanceof ApiRequestError && error.status === 401;
}

export const authStore = new AuthStore();
