import { parseRemotePath, sameSource } from '#lib/api/remotePaths.js';
import type { FileInfo } from '#lib/api/files.js';
import { desktopInputPath } from './desktopPath';
import type { TransferOperation, TransferSource } from './transfers';

/** Items dragged out of a BoxBox file view. Other apps see `text/plain` paths. */
export const FILE_DRAG_TYPE = 'application/x-boxbox-files';

export type FileDragKind = 'files' | 'external';

/** Performs drops for one pane. */
export interface FileDropHandler {
	/** Accepts the drag over a destination: prevents the default and sets the drop effect. */
	over(event: DragEvent, destination: string): boolean;
	drop(event: DragEvent, destination: string): void;
}

/** An area (pane, folder rows, sidebar) that highlights the destination under the pointer. */
export interface FileDropTarget {
	readonly overPath: string | null;
	/** Returns whether the destination accepts the drag; a refused drag can bubble further. */
	over(event: DragEvent, destination: string): boolean;
	leave(event: DragEvent): void;
	/** Refuses the drag here and drops any highlight, without letting it reach the pane. */
	refuse(event: DragEvent): void;
	drop(event: DragEvent, destination: string): void;
}

// Dragover cannot read DataTransfer data, so the sources of a drag that started here are kept.
let dragging: TransferSource[] | null = null;

export function startFileDrag(dataTransfer: DataTransfer, items: readonly FileInfo[]): void {
	dragging = items.map(({ path, name, isDir }) => ({ path, name, isDir }));
	dataTransfer.effectAllowed = 'copyMove';
	dataTransfer.setData(FILE_DRAG_TYPE, JSON.stringify(dragging));
	dataTransfer.setData('text/plain', items.map((item) => item.path).join('\n'));
}

export function endFileDrag(): void {
	dragging = null;
}

export function draggedSources(): readonly TransferSource[] | null {
	return dragging;
}

/** Whether the item is part of the drag in progress, so dropping on it means nothing. */
export function isDragged(path: string): boolean {
	return dragging?.some((source) => source.path === path) ?? false;
}

export function fileDragKind(dataTransfer: DataTransfer | null): FileDragKind | null {
	if (!dataTransfer) return null;
	const types = Array.from(dataTransfer.types);
	if (types.includes(FILE_DRAG_TYPE)) return 'files';
	if (types.includes('Files') || types.includes('text/uri-list')) return 'external';
	return null;
}

/** Files from other apps are copied; BoxBox items move unless Ctrl is held. */
export function dropOperation(
	kind: FileDragKind,
	event: { ctrlKey: boolean },
	destination?: string,
	sources: readonly TransferSource[] | null = dragging
): TransferOperation {
	const crossSource =
		destination !== undefined &&
		sources?.some(
			(source) => !sameSource(parseRemotePath(source.path), parseRemotePath(destination))
		);
	return kind === 'external' || event.ctrlKey || crossSource ? 'copy' : 'move';
}

function parentPath(path: string): string {
	return path.slice(0, Math.max(0, path.lastIndexOf('/')));
}

/** Refuses drops that cannot work or do nothing: into the item itself, or moved to where it is. */
export function canDropInto(
	sources: readonly TransferSource[] | null,
	destination: string,
	operation: TransferOperation
): boolean {
	if (!destination) return false;
	if (!sources) return true;
	if (
		sources.some(
			(source) => destination === source.path || destination.startsWith(`${source.path}/`)
		)
	)
		return false;
	return !(
		operation === 'move' && sources.every((source) => parentPath(source.path) === destination)
	);
}

export function readFileDrag(dataTransfer: DataTransfer): TransferSource[] {
	let value: unknown;
	try {
		value = JSON.parse(dataTransfer.getData(FILE_DRAG_TYPE));
	} catch {
		return [];
	}
	if (!Array.isArray(value)) return [];
	return value.flatMap((item: unknown) => {
		if (typeof item !== 'object' || item === null) return [];
		const path = 'path' in item ? item.path : undefined;
		const name = 'name' in item ? item.name : undefined;
		const isDir = 'isDir' in item ? item.isDir : undefined;
		if (typeof path !== 'string' || typeof name !== 'string' || typeof isDir !== 'boolean')
			return [];
		try {
			parseRemotePath(path);
		} catch {
			return [];
		}
		return [{ path, name, isDir }];
	});
}

/** Local files another app dropped, as BoxBox paths. Only the desktop app can reach them. */
export function readDroppedLocalFiles(uriList: string): TransferSource[] {
	return uriList
		.split(/\r?\n/)
		.map((line) => line.trim())
		.filter((line) => line.startsWith('file:'))
		.flatMap((uri) => {
			let path: string;
			try {
				path = desktopInputPath(uri);
			} catch {
				return [];
			}
			// A mount itself, such as the file system root, cannot be copied into a folder.
			if (!path.includes('/')) return [];
			const name = path.split('/').at(-1);
			return name ? [{ path, name, isDir: false }] : [];
		});
}
