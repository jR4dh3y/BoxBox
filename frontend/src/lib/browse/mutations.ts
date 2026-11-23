import { isNativePath } from '#lib/api/desktop.js';
import { createDirectory, createFile, deleteFile, rename, type FileInfo } from '#lib/api/files.js';
import { createDeleteJob } from '#lib/api/jobs.js';
import { jobsStore } from '#lib/stores/jobs.svelte.js';

export function createItem(type: 'file' | 'directory', folder: string, name: string) {
	return type === 'file' ? createFile(folder, name) : createDirectory(folder, name);
}

export async function renameItem(oldPath: string, newPath: string): Promise<void> {
	await rename(oldPath, newPath);
	if (isNativePath(oldPath)) {
		const { desktopUndo } = await import('./undo.svelte');
		desktopUndo.rememberRename(oldPath, newPath);
	}
}

/** Files go at once; server folders become background jobs. */
export async function deleteItems(items: readonly FileInfo[]): Promise<void> {
	const trashed: string[] = [];
	for (const item of items) {
		if (isNativePath(item.path)) {
			const { trashDesktopFile } = await import('#lib/api/desktopTrash.js');
			trashed.push((await trashDesktopFile(item.path)).id);
			const { desktopUndo } = await import('./undo.svelte');
			desktopUndo.rememberTrash([...trashed]);
		} else if (item.isDir) {
			jobsStore.upsertJob(await createDeleteJob(item.path));
		} else {
			await deleteFile(item.path);
		}
	}
}
