import { isNativePath } from '#lib/api/desktop.js';
import { createDirectory, createFile, deleteFile, rename, type FileInfo } from '#lib/api/files.js';
import { createDeleteJob } from '#lib/api/jobs.js';
import { jobsStore } from '#lib/stores/jobs.svelte.js';

export function createItem(
	type: 'file' | 'directory',
	folder: string,
	name: string,
	signal: AbortSignal
) {
	return type === 'file'
		? createFile(folder, name, '', signal)
		: createDirectory(folder, name, signal);
}

export async function renameItem(
	oldPath: string,
	newPath: string,
	signal: AbortSignal
): Promise<void> {
	await rename(oldPath, newPath, signal);
	if (isNativePath(oldPath)) {
		const { desktopUndo } = await import('./undo.svelte');
		if (!signal.aborted) desktopUndo.rememberRename(oldPath, newPath);
	}
}

/** Files go at once; server folders become background jobs. */
export async function deleteItems(items: readonly FileInfo[], signal: AbortSignal): Promise<void> {
	const trashed: string[] = [];
	for (const item of items) {
		signal.throwIfAborted();
		if (isNativePath(item.path)) {
			const { trashDesktopFile } = await import('#lib/api/desktopTrash.js');
			trashed.push((await trashDesktopFile(item.path, signal)).id);
			const { desktopUndo } = await import('./undo.svelte');
			if (!signal.aborted) desktopUndo.rememberTrash([...trashed]);
		} else if (item.isDir) {
			const job = await createDeleteJob(item.path, signal);
			if (!signal.aborted) jobsStore.upsertJob(job);
		} else {
			await deleteFile(item.path, false, signal);
		}
	}
}
