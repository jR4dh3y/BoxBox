import type { FileInfo } from '#lib/api/files.js';

const THUMBNAIL_EXTENSIONS = new Set(['jpg', 'jpeg', 'png', 'gif', 'webp']);
const THUMBNAIL_MIME_TYPES = new Set(['image/jpeg', 'image/png', 'image/gif', 'image/webp']);

export function getThumbnailSourceKey(file: Pick<FileInfo, 'path' | 'modTime' | 'size'>): string {
	return JSON.stringify([file.path, file.modTime, file.size]);
}

export function supportsThumbnailFile(
	file: Pick<FileInfo, 'path' | 'mimeType' | 'isDir'>
): boolean {
	if (file.isDir) return false;
	if (file.mimeType && THUMBNAIL_MIME_TYPES.has(file.mimeType.toLowerCase())) return true;
	const extension = file.path.split('/').pop()?.split('.').pop()?.toLowerCase();
	return extension !== undefined && THUMBNAIL_EXTENSIONS.has(extension);
}
