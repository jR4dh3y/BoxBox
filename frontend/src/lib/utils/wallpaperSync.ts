/**
 * Keeps the wallpaper stored on the server, which share pages show, matching the
 * wallpaper the signed-in user chose. Every sync runs through one queue and reads
 * the settings when it runs, so a slow login sync cannot overwrite a later save.
 */
import {
	deleteWallpaper,
	getWallpaper,
	updateWallpaperDisplay,
	uploadWallpaper
} from '$lib/api/wallpaper';
import { settingsStore } from '$lib/stores/settings.svelte';
import { resolveBackgroundImageUrl } from '$lib/utils/appearance';
import { currentUsername } from '$lib/utils/sessionUser';
import { normalizeBackgroundImageMode } from '$lib/utils/wallpaper';
import {
	dataUrlToBlob,
	getLocalWallpaperBlob,
	isInlineWallpaperDataUrl,
	isLocalWallpaperReference
} from '$lib/utils/wallpaperStorage';

// Browser settings are not per account, so remember whose wallpaper this is.
const OWNER_KEY = 'boxbox_wallpaper_owner';

let queue: Promise<void> = Promise.resolve();
let loginSyncStarted = false;
let sessionController = new AbortController();

function enqueue(task: () => Promise<void>): Promise<void> {
	const run = queue.then(task);
	queue = run.catch(() => {});
	return run;
}

/**
 * After sign-in, upload this browser's wallpaper if it belongs to the signed-in
 * user and the server copy differs. Never deletes: a browser without a wallpaper
 * says nothing about the user's choice elsewhere.
 */
export function syncShareWallpaperOnLogin(): Promise<void> {
	if (loginSyncStarted) return Promise.resolve();
	loginSyncStarted = true;
	const { signal } = sessionController;
	return enqueue(async () => {
		signal.throwIfAborted();
		const user = currentUsername();
		const owner = readOwner();
		const { backgroundImage } = settingsStore.current;
		if (!user || !backgroundImage || (owner !== null && owner !== user)) return;
		await reconcile(backgroundImage, signal);
		signal.throwIfAborted();
		writeOwner(user);
	}).catch((error) => {
		if (!signal.aborted) console.warn('Unable to sync the share page wallpaper', error);
	});
}

/** Allow the next authenticated session to sync, including after expiry. */
export function resetShareWallpaperLoginSync(): void {
	sessionController.abort();
	sessionController = new AbortController();
	loginSyncStarted = false;
}

/** After saving settings: make the server copy match exactly, including removal. */
export function publishShareWallpaper(): Promise<void> {
	const { signal } = sessionController;
	return enqueue(async () => {
		signal.throwIfAborted();
		const user = currentUsername();
		const { backgroundImage } = settingsStore.current;
		if (backgroundImage) {
			await reconcile(backgroundImage, signal);
		} else if (await getWallpaper(signal)) {
			signal.throwIfAborted();
			await deleteWallpaper(signal);
		}
		signal.throwIfAborted();
		if (user) writeOwner(user);
	});
}

async function reconcile(backgroundImage: string, signal: AbortSignal) {
	const display = {
		mode: normalizeBackgroundImageMode(settingsStore.current.backgroundImageMode),
		frostedGlass: settingsStore.current.frostedGlass
	};
	const [stored, image] = await Promise.all([
		getWallpaper(signal),
		readWallpaperImage(backgroundImage, signal)
	]);
	signal.throwIfAborted();
	if (!image) throw new Error('The wallpaper image could not be read for sharing.');

	// Compare bytes when the browser can hash (secure origins): a server image
	// replaced at the same path keeps its source string.
	const hash = await sha256Hex(image);
	signal.throwIfAborted();
	const sameImage =
		stored !== null &&
		stored.source === backgroundImage &&
		(hash === null || hash === stored.sha256);
	if (!sameImage) {
		await uploadWallpaper(image, display, backgroundImage, signal);
	} else if (stored.mode !== display.mode || stored.frostedGlass !== display.frostedGlass) {
		await updateWallpaperDisplay(display, signal);
	}
}

// Device wallpapers are read straight from browser storage: the page's CSP does
// not allow fetching blob: URLs. Server wallpapers come from the preview API.
async function readWallpaperImage(
	backgroundImage: string,
	signal: AbortSignal
): Promise<Blob | null> {
	if (isLocalWallpaperReference(backgroundImage)) return getLocalWallpaperBlob(backgroundImage);
	if (isInlineWallpaperDataUrl(backgroundImage)) return dataUrlToBlob(backgroundImage);

	const url = await resolveBackgroundImageUrl(backgroundImage);
	signal.throwIfAborted();
	const response = url ? await fetch(url, { signal }) : null;
	return response?.ok ? response.blob() : null;
}

async function sha256Hex(blob: Blob): Promise<string | null> {
	if (!globalThis.crypto?.subtle) return null;
	const digest = await crypto.subtle.digest('SHA-256', await blob.arrayBuffer());
	return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, '0')).join('');
}

function readOwner(): string | null {
	try {
		return localStorage.getItem(OWNER_KEY);
	} catch {
		return null;
	}
}

function writeOwner(user: string) {
	try {
		localStorage.setItem(OWNER_KEY, user);
	} catch {
		// Without the marker the next login sync simply checks again.
	}
}
