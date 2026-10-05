import { ApiRequestError, createShareFolder, shareUploadUrl } from '$lib/api';
import { foldersFor, type UploadEntry } from '$lib/utils/uploadEntries';

/** Uploads a batch with the share identity captured before reading local files. */
export async function uploadShareEntries({
	token,
	path,
	entries,
	signal,
	onFile,
	onProgress
}: {
	token: string;
	path: string;
	entries: UploadEntry[];
	signal: AbortSignal;
	onFile: (name: string) => void;
	onProgress: (percent: number) => void;
}) {
	const destination = (relativePath: string) => (path ? `${path}/${relativePath}` : relativePath);
	for (const folder of foldersFor(entries)) {
		signal.throwIfAborted();
		await createShareFolder(token, destination(folder), signal);
	}
	for (const [index, entry] of entries.entries()) {
		signal.throwIfAborted();
		onFile(
			entries.length > 1
				? `${entry.relativePath} (${index + 1} of ${entries.length})`
				: entry.relativePath
		);
		await uploadShareFile({
			token,
			path: destination(entry.relativePath),
			file: entry.file,
			signal,
			onProgress
		});
	}
}

/** Sends one file or editor save, with progress and cancellable network I/O. */
export function uploadShareFile({
	token,
	path,
	file,
	signal,
	onProgress
}: {
	token: string;
	path: string;
	file: Blob;
	signal?: AbortSignal;
	onProgress: (percent: number) => void;
}): Promise<void> {
	onProgress(0);
	return new Promise((resolve, reject) => {
		if (signal?.aborted) {
			reject(signal.reason);
			return;
		}
		const xhr = new XMLHttpRequest();
		const abort = () => xhr.abort();
		const finish = (error?: Error) => {
			signal?.removeEventListener('abort', abort);
			if (error) reject(error);
			else resolve();
		};
		xhr.open('POST', shareUploadUrl(token, path));
		xhr.upload.onprogress = (event) => {
			if (event.lengthComputable) onProgress(Math.round((event.loaded / event.total) * 100));
		};
		xhr.onload = () => {
			if (xhr.status >= 200 && xhr.status < 300) finish();
			else if (xhr.status === 404) finish(new ApiRequestError('Share not found', 404, 'NOT_FOUND'));
			else if (xhr.status === 403) finish(new Error('This link does not allow this upload.'));
			else if (xhr.status === 413) finish(new Error('File too large'));
			else finish(new Error('Unable to add this file'));
		};
		xhr.onerror = () => finish(new Error('Unable to add this file'));
		xhr.onabort = () => finish(new DOMException('Upload cancelled', 'AbortError'));
		signal?.addEventListener('abort', abort, { once: true });
		try {
			xhr.send(file);
		} catch (error) {
			finish(error instanceof Error ? error : new Error('Unable to add this file'));
		}
	});
}
