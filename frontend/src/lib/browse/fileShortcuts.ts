export type FileShortcut =
	| 'select-all'
	| 'clear-selection'
	| 'search'
	| 'details'
	| 'open'
	| 'new-folder'
	| 'copy'
	| 'cut'
	| 'paste'
	| 'rename'
	| 'delete'
	| 'delete-permanently';

/** Text fields keep their own keys; selection checkboxes do not. */
export function isTextEntryTarget(target: EventTarget | null): boolean {
	if (!(target instanceof HTMLElement)) return false;
	if (target instanceof HTMLInputElement && target.type === 'checkbox') return false;
	return (
		target.isContentEditable ||
		target.closest('input, textarea, select, [contenteditable="true"]') !== null
	);
}

/** Controls that Enter already activates; file rows handle their own Enter. */
export function isActivatableTarget(target: EventTarget | null): boolean {
	return (
		target instanceof Element &&
		target.closest('button, a[href], summary, [role="button"], [role="tab"], [role="link"]') !==
			null
	);
}

export function getFileShortcut(
	event: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>
): FileShortcut | null {
	const key = event.key.toLowerCase();
	const command = event.ctrlKey || event.metaKey;
	if (command && !event.altKey) {
		if (event.shiftKey) return key === 'n' ? 'new-folder' : null;
		switch (key) {
			case 'a':
				return 'select-all';
			case 'c':
				return 'copy';
			case 'x':
				return 'cut';
			case 'v':
				return 'paste';
			case 'f':
				return 'search';
		}
	}
	if (command) return null;
	if (event.altKey) return !event.shiftKey && key === 'enter' ? 'details' : null;
	if (key === 'delete') return event.shiftKey ? 'delete-permanently' : 'delete';
	if (event.shiftKey) return null;
	if (key === 'f2') return 'rename';
	if (key === 'escape') return 'clear-selection';
	if (key === 'enter') return 'open';
	return null;
}

export function fileNavigationIndex(
	key: string,
	index: number,
	count: number,
	columns = 1
): number | null {
	if (count === 0 || index < 0) return null;
	switch (key) {
		case 'ArrowDown':
			return Math.min(count - 1, index + columns);
		case 'ArrowUp':
			return Math.max(0, index - columns);
		case 'ArrowRight':
			return Math.min(count - 1, index + 1);
		case 'ArrowLeft':
			return Math.max(0, index - 1);
		case 'Home':
			return 0;
		case 'End':
			return count - 1;
		default:
			return null;
	}
}
