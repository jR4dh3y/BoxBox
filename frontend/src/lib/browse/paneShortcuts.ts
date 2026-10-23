export type BrowseShortcutAction =
	| 'new-tab'
	| 'close-tab'
	| 'next-tab'
	| 'previous-tab'
	| 'toggle-split'
	| 'back'
	| 'forward'
	| 'up'
	| 'refresh'
	| 'toggle-hidden'
	| 'focus-location'
	| 'open-servers';

export interface BrowseShortcutInput {
	key: string;
	ctrlKey: boolean;
	metaKey: boolean;
	shiftKey: boolean;
	altKey: boolean;
	editable: boolean;
}

/** Maps app shortcuts before the page prevents the browser's default action. */
export function getBrowseShortcut(input: BrowseShortcutInput): BrowseShortcutAction | null {
	const command = input.ctrlKey || input.metaKey;
	const key = input.key.toLowerCase();

	if (command && !input.altKey) {
		if (key === 't' && !input.shiftKey) return 'new-tab';
		if (key === 'w' && !input.shiftKey) return 'close-tab';
		if (key === 'tab') return input.shiftKey ? 'previous-tab' : 'next-tab';
		if (key === 'l') return 'focus-location';
		if (key === 'o' && input.shiftKey) return 'open-servers';
		if (key === 'h' && !input.shiftKey && !input.editable) return 'toggle-hidden';
	}

	if (input.editable) return null;
	if (!command && !input.altKey && !input.shiftKey && input.key === 'F3') return 'toggle-split';
	if (!command && !input.altKey && !input.shiftKey && input.key === 'F5') return 'refresh';
	if (input.altKey && !command && !input.shiftKey) {
		if (input.key === 'ArrowLeft') return 'back';
		if (input.key === 'ArrowRight') return 'forward';
		if (input.key === 'ArrowUp') return 'up';
	}
	return null;
}
