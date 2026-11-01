import type { FileInfo } from '#lib/api/files.js';
import { isNativePath } from '#lib/api/desktop.js';
import { getPreviewType } from './fileTypes';

const IMAGE = /(?:^image\/|\.)(jpe?g|png|gif|webp)$/i;

export function getThumbnailSourceKey(file: Pick<FileInfo, 'path' | 'modTime' | 'size'>): string {
	return JSON.stringify([file.path, file.modTime, file.size]);
}

export function supportsThumbnailFile(
	file: Pick<FileInfo, 'path' | 'mimeType' | 'isDir'>
): boolean {
	if (file.isDir) return false;
	if (IMAGE.test(file.path) || IMAGE.test(file.mimeType ?? '')) return true;
	if (!isNativePath(file.path)) return false;
	const type = getPreviewType(file.path);
	return type === 'image'
		? !/\.(svg|avif|raw)$/i.test(file.path)
		: /^(video|audio|pdf)$/.test(type);
}
