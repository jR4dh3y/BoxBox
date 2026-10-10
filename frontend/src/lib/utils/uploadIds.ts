import { CONFIG } from '$lib/config';

/** Kept apart from the transfer code so the upload store can name and size uploads without loading it. */
export function generateUploadId(): string {
	return `upload_${Date.now()}_${Math.random().toString(36).substring(2, 11)}`;
}

export function getChunkCount(
	fileSize: number,
	chunkSize: number = CONFIG.upload.defaultChunkSize
): number {
	return Math.ceil(fileSize / chunkSize);
}
