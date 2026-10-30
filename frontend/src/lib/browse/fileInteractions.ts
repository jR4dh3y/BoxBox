import type { FileOpenBehavior } from '#lib/utils/fileOpenBehavior.js';

export type FileSelectionGesture =
	| { type: 'replace' }
	| { type: 'toggle' }
	| { type: 'range' }
	| { type: 'add-range' }
	| { type: 'set'; selected: boolean };

export interface FileSelectionModifiers {
	ctrlKey: boolean;
	metaKey: boolean;
	shiftKey: boolean;
}

export interface FileSelectionResult {
	paths: Set<string>;
	anchorPath: string;
}

export function getSelectedContextMenuItems<Item extends { path: string }>(
	items: readonly Item[],
	selectedPaths: ReadonlySet<string>,
	clickedItem: Item
): Item[] {
	return selectedPaths.has(clickedItem.path)
		? items.filter((item) => selectedPaths.has(item.path))
		: [clickedItem];
}

export function fileSelectionGestureFromModifiers(
	modifiers: FileSelectionModifiers
): FileSelectionGesture {
	if (modifiers.shiftKey) {
		return { type: modifiers.ctrlKey || modifiers.metaKey ? 'add-range' : 'range' };
	}
	return { type: modifiers.ctrlKey || modifiers.metaKey ? 'toggle' : 'replace' };
}

/** Apply a selection gesture while keeping Shift ranges anchored to the first item. */
export function applyFileSelection(
	orderedPaths: readonly string[],
	selectedPaths: ReadonlySet<string>,
	anchorPath: string | null,
	targetPath: string,
	gesture: FileSelectionGesture
): FileSelectionResult {
	const targetIndex = orderedPaths.indexOf(targetPath);
	if (targetIndex < 0) {
		return { paths: new Set(selectedPaths), anchorPath: anchorPath ?? targetPath };
	}

	const existingAnchorIndex = anchorPath === null ? -1 : orderedPaths.indexOf(anchorPath);
	const nextAnchorPath = existingAnchorIndex < 0 || anchorPath === null ? targetPath : anchorPath;

	if (gesture.type === 'replace') {
		return { paths: new Set([targetPath]), anchorPath: targetPath };
	}

	if (gesture.type === 'toggle' || gesture.type === 'set') {
		const paths = new Set(selectedPaths);
		const shouldSelect = gesture.type === 'toggle' ? !paths.has(targetPath) : gesture.selected;
		if (shouldSelect) paths.add(targetPath);
		else paths.delete(targetPath);
		return { paths, anchorPath: targetPath };
	}

	const rangeStart = Math.min(
		existingAnchorIndex < 0 ? targetIndex : existingAnchorIndex,
		targetIndex
	);
	const rangeEnd = Math.max(
		existingAnchorIndex < 0 ? targetIndex : existingAnchorIndex,
		targetIndex
	);
	const paths = gesture.type === 'range' ? new Set<string>() : new Set(selectedPaths);
	for (const path of orderedPaths.slice(rangeStart, rangeEnd + 1)) paths.add(path);

	return { paths, anchorPath: nextAnchorPath };
}

type ClickModifiers = Pick<MouseEvent, 'ctrlKey' | 'metaKey' | 'shiftKey'>;

function isUnmodifiedClick(event: ClickModifiers): boolean {
	return !event.ctrlKey && !event.metaKey && !event.shiftKey;
}

export function shouldOpenOnClick(
	behavior: FileOpenBehavior,
	event: ClickModifiers & Pick<MouseEvent, 'detail'>
): boolean {
	return behavior === 'single-click' && event.detail === 1 && isUnmodifiedClick(event);
}

export function shouldOpenOnDoubleClick(
	behavior: FileOpenBehavior,
	event: ClickModifiers
): boolean {
	return behavior === 'double-click' && isUnmodifiedClick(event);
}
