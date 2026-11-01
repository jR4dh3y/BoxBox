import { getUploadConfig } from '#lib/api/system.js';
import { parseRemotePath } from '#lib/api/remotePaths.js';

const pending = new Map<string, Promise<number>>();

/** Only uploads starting together share configuration; later uploads confirm server settings. */
export function uploadChunkSize(destination: string): Promise<number> {
	const source = parseRemotePath(destination);
	const key = source.kind === 'remote' ? source.serverId : 'local';
	const existing = pending.get(key);
	if (existing) return existing;
	const request = getUploadConfig(destination).then((config) => {
		if (!Number.isSafeInteger(config.chunkSizeBytes) || config.chunkSizeBytes <= 0)
			throw new Error('Invalid upload chunk size');
		return config.chunkSizeBytes;
	});
	pending.set(key, request);
	void request
		.finally(() => {
			if (pending.get(key) === request) pending.delete(key);
		})
		.catch(() => {});
	return request;
}
