import { SvelteSet } from 'svelte/reactivity';
import type { FileInfo } from '#lib/api/files.js';
import type { FileContextAction } from '#lib/utils/fileContextMenu.js';
import type { FileOpenBehavior } from '#lib/utils/fileOpenBehavior.js';
import {
	applyFileSelection,
	fileSelectionGestureFromModifiers,
	getSelectedContextMenuItems,
	shouldOpenOnClick,
	shouldOpenOnDoubleClick,
	type FileSelectionGesture
} from './fileInteractions';
import { fileNavigationIndex } from './fileShortcuts';

export interface FileCollectionProps {
	items: FileInfo[];
	selectedPaths: ReadonlySet<string>;
	fileOpenBehavior: FileOpenBehavior;
	onItemClick?: (item: FileInfo) => void;
	onSelectionChange?: (paths: Set<string>) => void;
	onContextMenuAction?: (action: FileContextAction, items: FileInfo[]) => void;
}

/** The parts of a key press that item navigation reads. */
export type FileKeyInput = Pick<
	KeyboardEvent,
	'key' | 'altKey' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'target' | 'preventDefault'
>;

/** How the view lays items out: arrow keys move by `columns`, and `focus` brings an index into view. */
export interface FileCollectionView {
	readonly columns: number;
	/** Where navigation resumes once the focused item has scrolled out. */
	readonly focusedPath: string | null;
	focus(index: number): Promise<void>;
}

/**
 * Pointer and keyboard selection shared by the list and grid views.
 * File actions (copy, delete, rename…) are pane shortcuts, not item shortcuts.
 * Navigation works on positions in `items`, so it reaches items the view has not rendered.
 */
export function useFileCollection(props: () => FileCollectionProps, view: FileCollectionView) {
	let anchor: string | null = null;
	let contextMenu = $state<{ x: number; y: number; items: FileInfo[] } | null>(null);

	function select(path: string, gesture: FileSelectionGesture): void {
		const { items, selectedPaths, onSelectionChange } = props();
		const result = applyFileSelection(
			items.map((item) => item.path),
			selectedPaths,
			anchor,
			path,
			gesture
		);
		anchor = result.anchorPath;
		onSelectionChange?.(new SvelteSet(result.paths));
	}

	function openMenu(event: MouseEvent, items: FileInfo[]): void {
		event.preventDefault();
		contextMenu = { x: event.clientX, y: event.clientY, items };
	}

	function keydown(item: FileInfo, event: FileKeyInput): void {
		if (
			event.target instanceof HTMLInputElement &&
			(event.target.type !== 'checkbox' || event.key === ' ' || event.key === 'Enter')
		)
			return;

		const { items } = props();
		const nextIndex = event.altKey
			? null
			: fileNavigationIndex(
					event.key,
					items.findIndex((candidate) => candidate.path === item.path),
					items.length,
					view.columns
				);
		if (nextIndex !== null) {
			event.preventDefault();
			void view.focus(nextIndex);
			if (!event.ctrlKey && !event.metaKey) {
				anchor ??= item.path;
				select(items[nextIndex].path, event.shiftKey ? { type: 'range' } : { type: 'replace' });
			}
			return;
		}
		if (event.altKey) return;
		if (event.key === 'Enter') {
			event.preventDefault();
			props().onItemClick?.(item);
		} else if (event.key === ' ') {
			event.preventDefault();
			select(item.path, fileSelectionGestureFromModifiers(event));
		}
	}

	return {
		get contextMenu() {
			return contextMenu;
		},
		click(item: FileInfo, event: MouseEvent): void {
			select(item.path, fileSelectionGestureFromModifiers(event));
			if (shouldOpenOnClick(props().fileOpenBehavior, event)) props().onItemClick?.(item);
		},
		doubleClick(item: FileInfo, event: MouseEvent): void {
			if (shouldOpenOnDoubleClick(props().fileOpenBehavior, event)) props().onItemClick?.(item);
		},
		check(item: FileInfo, selected: boolean): void {
			select(item.path, { type: 'set', selected });
		},
		/** Keys pressed on the scroller itself, after the focused item scrolled out of view. */
		scrollerKeydown(event: FileKeyInput & Pick<KeyboardEvent, 'currentTarget'>): void {
			if (event.target !== event.currentTarget) return;
			const { items } = props();
			const item = items.find((candidate) => candidate.path === view.focusedPath);
			if (item) keydown(item, event);
			// The item is gone, so there is no position to move from: start at the first.
			else if (fileNavigationIndex(event.key, 0, items.length) !== null) {
				event.preventDefault();
				void view.focus(0);
			}
		},
		keydown,
		itemContextMenu(item: FileInfo, event: MouseEvent): void {
			const { items, selectedPaths, onSelectionChange } = props();
			if (!selectedPaths.has(item.path)) {
				anchor = item.path;
				onSelectionChange?.(new SvelteSet([item.path]));
			}
			openMenu(event, getSelectedContextMenuItems(items, selectedPaths, item));
		},
		/** `itemSelector` matches elements that own their own context menu. */
		backgroundContextMenu(event: MouseEvent, itemSelector: string): void {
			if (event.target instanceof Element && event.target.closest(itemSelector)) return;
			anchor = null;
			props().onSelectionChange?.(new SvelteSet());
			openMenu(event, []);
		},
		selectMenuAction(action: FileContextAction): void {
			if (contextMenu) props().onContextMenuAction?.(action, contextMenu.items);
			contextMenu = null;
		},
		closeMenu(): void {
			contextMenu = null;
		}
	};
}

export function fileDisplayName(item: FileInfo, showExtension: boolean): string {
	if (showExtension || item.isDir) return item.name;
	const dotIndex = item.name.lastIndexOf('.');
	return dotIndex > 0 ? item.name.slice(0, dotIndex) : item.name;
}
