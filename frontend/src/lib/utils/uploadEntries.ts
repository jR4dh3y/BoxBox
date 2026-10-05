/**
 * Turn picked or dropped files (including whole folders) into upload entries
 * that keep each file's path relative to where it was dropped.
 */

export interface UploadEntry {
	file: File;
	/** Path below the upload destination, e.g. "photos/2024/a.jpg". */
	relativePath: string;
}

/** Entries from an <input type="file">; folder pickers fill webkitRelativePath. */
export function entriesFromFiles(files: Iterable<File>): UploadEntry[] {
	return Array.from(files, (file) => ({
		file,
		relativePath: file.webkitRelativePath || file.name
	}));
}

/** Entries from a drop, walking into any dropped folders. */
export async function entriesFromDataTransfer(dataTransfer: DataTransfer): Promise<UploadEntry[]> {
	const roots = Array.from(dataTransfer.items)
		.filter((item) => item.kind === 'file')
		.map((item) => item.webkitGetAsEntry?.())
		.filter((entry): entry is FileSystemEntry => Boolean(entry));
	if (roots.length === 0) return entriesFromFiles(dataTransfer.files);

	const entries: UploadEntry[] = [];
	for (const root of roots) await collectEntries(root, '', entries);
	return entries;
}

async function collectEntries(entry: FileSystemEntry, prefix: string, entries: UploadEntry[]) {
	const relativePath = prefix ? `${prefix}/${entry.name}` : entry.name;
	if (entry.isFile) {
		const file = await new Promise<File>((resolve, reject) =>
			(entry as FileSystemFileEntry).file(resolve, reject)
		);
		entries.push({ file, relativePath });
		return;
	}
	if (!entry.isDirectory) return;

	const reader = (entry as FileSystemDirectoryEntry).createReader();
	// readEntries returns results in batches; keep reading until it returns none.
	for (;;) {
		const batch = await new Promise<FileSystemEntry[]>((resolve, reject) =>
			reader.readEntries(resolve, reject)
		);
		if (batch.length === 0) break;
		for (const child of batch) await collectEntries(child, relativePath, entries);
	}
}
