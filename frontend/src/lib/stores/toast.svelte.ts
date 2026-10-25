/**
 * Toast notification store using Svelte 5 runes
 * Provides success, error, info, warning notifications
 */

import { untrack } from 'svelte';

export type ToastType = 'success' | 'error' | 'info' | 'warning';

export interface Toast {
	id: string;
	type: ToastType;
	message: string;
	duration: number;
}

const DEFAULT_DURATION = 5000; // 5 seconds

/**
 * Generate unique toast ID
 */
function generateId(): string {
	return `toast_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`;
}

/**
 * Toast store class using Svelte 5 runes
 */
class ToastStore {
	toasts = $state<Toast[]>([]);

	private timeouts = new Map<string, ReturnType<typeof setTimeout>>();

	/**
	 * Add a toast notification
	 */
	add(type: ToastType, message: string, duration: number = DEFAULT_DURATION): string {
		const id = generateId();
		const toast: Toast = { id, type, message, duration };

		this.toasts = [...this.toasts, toast];

		return id;
	}

	/** Start the visible lifetime after the lazy renderer attaches the message. */
	startLifetime(id: string): void {
		if (this.timeouts.has(id)) return;
		const toast = untrack(() => this.toasts.find((toast) => toast.id === id));
		if (toast && toast.duration > 0) {
			this.timeouts.set(
				id,
				setTimeout(() => this.remove(id), toast.duration)
			);
		}
	}

	/**
	 * Remove a toast by ID
	 */
	remove(id: string): void {
		// Clear timeout if exists
		const timeout = this.timeouts.get(id);
		if (timeout) {
			clearTimeout(timeout);
			this.timeouts.delete(id);
		}

		this.toasts = this.toasts.filter((t) => t.id !== id);
	}

	/**
	 * Clear all toasts
	 */
	clear(): void {
		// Clear all timeouts
		for (const timeout of this.timeouts.values()) {
			clearTimeout(timeout);
		}
		this.timeouts.clear();

		this.toasts = [];
	}

	/**
	 * Show success toast
	 */
	success(message: string, duration?: number): string {
		return this.add('success', message, duration);
	}

	/**
	 * Show error toast
	 */
	error(message: string, duration?: number): string {
		return this.add('error', message, duration);
	}

	/**
	 * Show info toast
	 */
	info(message: string, duration?: number): string {
		return this.add('info', message, duration);
	}

	/**
	 * Show warning toast
	 */
	warning(message: string, duration?: number): string {
		return this.add('warning', message, duration);
	}
}

/**
 * Singleton toast store instance
 */
export const toastStore = new ToastStore();
