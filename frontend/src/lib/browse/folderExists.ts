import { peekDirectory } from '#lib/api/files.js';
import { isRootPath } from './folderViews.js';

/**
 * Whether a folder can still be opened; the drives always can. A peek reads at most a few dozen
 * names, and fails for a path that is gone or is no longer a folder.
 */
export async function folderExists(path: string): Promise<boolean> {
	if (isRootPath(path)) return true;
	try {
		await peekDirectory(path, false);
		return true;
	} catch {
		return false;
	}
}
