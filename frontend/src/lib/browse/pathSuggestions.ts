import type { FileInfo } from '#lib/api/files.js';
import { visibleByName } from './sortFiles';

/** Child folders to suggest for `prefix`. A prefix starting with a dot asks for hidden folders. */
export function suggestedDirectories(
	items: readonly FileInfo[],
	prefix: string,
	includeHidden: boolean
): FileInfo[] {
	return visibleByName(
		items.filter((item) => item.isDir),
		includeHidden || prefix.startsWith('.')
	);
}
