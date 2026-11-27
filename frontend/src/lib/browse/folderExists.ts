import { ApiRequestError } from '#lib/api/client.js';
import { peekDirectory } from '#lib/api/files.js';
import { isRootPath } from './folderViews.js';

/**
 * Whether a folder can still be opened; the drives always can. A peek reads at most a few dozen
 * names. Only the server saying the path is gone (404) or is not a folder (400) counts as
 * missing; any other failure may be brief, so the folder is kept and opening it shows the error.
 */
export async function folderExists(path: string): Promise<boolean> {
	if (isRootPath(path)) return true;
	try {
		await peekDirectory(path, false);
		return true;
	} catch (error) {
		return !(error instanceof ApiRequestError && (error.status === 404 || error.status === 400));
	}
}
