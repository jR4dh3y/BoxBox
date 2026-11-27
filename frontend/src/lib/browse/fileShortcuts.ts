/** Keys the browse page binds while no text field, dialog or menu has them. */
export type FileShortcut =
	| 'select-all'
	| 'clear-selection'
	| 'select-pattern'
	| 'invert-selection'
	| 'search'
	| 'new-folder'
	| 'paste'
	| 'focus-location'
	| 'zoom-in'
	| 'zoom-out'
	| 'zoom-reset'
	| 'shortcuts'
	| 'details'
	| 'properties'
	| 'rename';

/** Text fields keep their own keys; selection checkboxes do not. */
export function isTextEntryTarget(target: EventTarget | null): boolean {
	if (!(target instanceof HTMLElement)) return false;
	if (target instanceof HTMLInputElement && target.type === 'checkbox') return false;
	return (
		target.isContentEditable ||
		target.closest('input, textarea, select, [contenteditable="true"]') !== null
	);
}

export function getFileShortcut(
	event: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>
): FileShortcut | null {
	const key = event.key.toLowerCase();
	const command = event.ctrlKey || event.metaKey;
	if (event.altKey)
		return !command && !event.shiftKey && event.key === 'Enter' ? 'properties' : null;
	if (command) {
		// Plus is Shift+= on most layouts, so zoom ignores Shift.
		if (key === '+' || key === '=') return 'zoom-in';
		if (key === '-' || key === '_') return 'zoom-out';
		if (event.shiftKey) return key === 'n' ? 'new-folder' : null;
		switch (key) {
			case 'a':
				return 'select-all';
			case 's':
				return 'select-pattern';
			case 'i':
				return 'invert-selection';
			case 'v':
				return 'paste';
			case 'f':
				return 'search';
			case 'l':
				return 'focus-location';
			case '0':
				return 'zoom-reset';
		}
		return null;
	}
	if (event.shiftKey) return null;
	if (event.key === 'F1') return 'shortcuts';
	if (event.key === 'F11') return 'details';
	if (event.key === 'F2') return 'rename';
	if (key === 'escape') return 'clear-selection';
	return null;
}
