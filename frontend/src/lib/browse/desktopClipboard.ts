import type { FileInfo } from '../api/files';
import {
	clearDesktopCutClipboard,
	getDesktopClipboard,
	setDesktopClipboard,
	type DesktopClipboardContents,
	type DesktopClipboardOperation
} from '../api/desktopClipboard';

type BrowseClipboardFile = Pick<FileInfo, 'path'>;

export function readBrowseClipboard(): Promise<DesktopClipboardContents | null> {
	return getDesktopClipboard();
}

export function writeBrowseClipboard(
	files: readonly BrowseClipboardFile[],
	operation: DesktopClipboardOperation
): Promise<void> {
	return setDesktopClipboard(
		files.map((file) => file.path),
		operation
	);
}

/** Call after the move job succeeds; copied files remain on the clipboard. */
export function clearBrowseCutClipboardAfterMove(
	clipboard: DesktopClipboardContents
): Promise<void> {
	if (clipboard.operation !== 'cut') return Promise.resolve();
	return clearDesktopCutClipboard(clipboard.items.map((item) => item.path));
}
