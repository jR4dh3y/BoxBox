/**
 * Clipboard store for managing cut/copy operations
 * Requirements: Context menu feature - clipboard state management
 */

import type { FileInfo } from '$lib/api/files';

export type ClipboardOperation = 'copy' | 'cut';

export interface ClipboardState {
	items: FileInfo[];
	operation: ClipboardOperation | null;
}

const state = $state<ClipboardState>({
	items: [],
	operation: null
});

/**
 * Clipboard store for managing file copy/cut operations
 */
export const clipboardStore = {
	/** Get current clipboard items */
	get items() {
		return state.items;
	},

	/** Get current operation type */
	get operation() {
		return state.operation;
	},

	/** Check if clipboard has items */
	get hasItems() {
		return state.items.length > 0;
	},

	/** Check if a specific path is in clipboard */
	isInClipboard(path: string): boolean {
		return state.items.some((item) => item.path === path);
	},

	/** Check if path is cut (for visual dimming) */
	isCut(path: string): boolean {
		return state.operation === 'cut' && this.isInClipboard(path);
	},

	/** Copy files to clipboard */
	copy(items: FileInfo[]) {
		state.items = [...items];
		state.operation = 'copy';
	},

	/** Cut files to clipboard */
	cut(items: FileInfo[]) {
		state.items = [...items];
		state.operation = 'cut';
	},

	/** Clear clipboard */
	clear() {
		state.items = [];
		state.operation = null;
	}
};
