export function getFileShortcut(
	event: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>
) {
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
	if (count === 0) return null;
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

/** Move within the rendered page; grid columns follow the actual responsive layout. */
export function moveFileFocus(event: KeyboardEvent, layout: 'list' | 'grid'): string | null {
	if (event.altKey || !(event.currentTarget instanceof HTMLElement)) return null;
	const current = event.currentTarget;
	const grid = current.closest('[role="grid"]');
	if (!grid) return null;
	const items = Array.from(grid.querySelectorAll<HTMLElement>('[data-file-path]'));
	const index = items.indexOf(current);
	if (index < 0) return null;
	const firstTop = items[0]?.getBoundingClientRect().top;
	const columns =
		layout === 'list'
			? 1
			: Math.max(1, items.filter((item) => item.getBoundingClientRect().top === firstTop).length);
	const nextIndex = fileNavigationIndex(event.key, index, items.length, columns);
	if (nextIndex === null) return null;
	event.preventDefault();
	const next = items[nextIndex];
	next?.focus();
	next?.scrollIntoView({ block: 'nearest' });
	return next?.getAttribute('data-file-path') ?? null;
}
