/**
 * Centralized localStorage access
 * Single point of access for all browser storage operations
 */

import { STORAGE_KEYS } from '$lib/config';

/**
 * Generic storage utilities
 */
export const storage = {
	/**
	 * Get a value from localStorage
	 */
	get<T>(key: string): T | null {
		if (typeof window === 'undefined') return null;
		try {
			const item = localStorage.getItem(key);
			return item ? JSON.parse(item) : null;
		} catch {
			return null;
		}
	},

	/**
	 * Get a raw string value from localStorage (no JSON parsing)
	 */
	getString(key: string): string | null {
		if (typeof window === 'undefined') return null;
		return localStorage.getItem(key);
	},

	/**
	 * Set a value in localStorage
	 */
	set<T>(key: string, value: T): void {
		if (typeof window === 'undefined') return;
		localStorage.setItem(key, JSON.stringify(value));
	},

	/**
	 * Set a raw string value in localStorage (no JSON stringification)
	 */
	setString(key: string, value: string): void {
		if (typeof window === 'undefined') return;
		localStorage.setItem(key, value);
	},

	/**
	 * Remove a value from localStorage
	 */
	remove(key: string): void {
		if (typeof window === 'undefined') return;
		localStorage.removeItem(key);
	},

	/**
	 * Check if a key exists in localStorage
	 */
	has(key: string): boolean {
		if (typeof window === 'undefined') return false;
		return localStorage.getItem(key) !== null;
	}
};

/**
 * Auth token storage - typed accessors
 */
export const tokenStorage = {
	getAccessToken(): string | null {
		return storage.getString(STORAGE_KEYS.ACCESS_TOKEN);
	},

	getRefreshToken(): string | null {
		return storage.getString(STORAGE_KEYS.REFRESH_TOKEN);
	},

	setTokens(accessToken: string, refreshToken: string): void {
		storage.setString(STORAGE_KEYS.ACCESS_TOKEN, accessToken);
		storage.setString(STORAGE_KEYS.REFRESH_TOKEN, refreshToken);
	},

	clearTokens(): void {
		storage.remove(STORAGE_KEYS.ACCESS_TOKEN);
		storage.remove(STORAGE_KEYS.REFRESH_TOKEN);
	},

	hasTokens(): boolean {
		return storage.has(STORAGE_KEYS.ACCESS_TOKEN);
	}
};

/**
 * Settings storage - typed accessors
 */
export const settingsStorage = {
	get<T>(): T | null {
		return storage.get<T>(STORAGE_KEYS.SETTINGS);
	},

	set<T>(settings: T): void {
		storage.set(STORAGE_KEYS.SETTINGS, settings);
	},

	clear(): void {
		storage.remove(STORAGE_KEYS.SETTINGS);
	}
};
