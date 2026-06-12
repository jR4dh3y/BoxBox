const DATABASE_NAME = 'boxbox-wallpapers';
const DATABASE_VERSION = 1;
const STORE_NAME = 'wallpapers';
const LOCAL_WALLPAPER_PREFIX = 'boxbox-local-wallpaper:';
const LOCAL_WALLPAPER_ID_PATTERN = /^[a-z0-9_-]{8,}$/i;
const INLINE_WALLPAPER_PATTERN = /^data:image\/(avif|gif|jpeg|jpg|png|svg\+xml|webp);base64,/i;

interface StoredLocalWallpaper {
	id: string;
	blob: Blob;
	type: string;
	createdAt: number;
}

const localWallpaperUrlCache = new Map<string, string>();

export function isInlineWallpaperDataUrl(value: string): boolean {
	return INLINE_WALLPAPER_PATTERN.test(value.trim());
}

export function toLocalWallpaperReference(id: string): string {
	return `${LOCAL_WALLPAPER_PREFIX}${id}`;
}

export function getLocalWallpaperId(reference: string): string | null {
	const trimmed = reference.trim();
	if (!trimmed.startsWith(LOCAL_WALLPAPER_PREFIX)) return null;

	const id = trimmed.slice(LOCAL_WALLPAPER_PREFIX.length);
	return LOCAL_WALLPAPER_ID_PATTERN.test(id) ? id : null;
}

export function isLocalWallpaperReference(value: string): boolean {
	return getLocalWallpaperId(value) !== null;
}

export async function saveLocalWallpaperDataUrl(dataUrl: string): Promise<string> {
	if (!isInlineWallpaperDataUrl(dataUrl)) {
		throw new Error('Only image data URLs can be saved as local wallpapers.');
	}

	const id = createLocalWallpaperId();
	const blob = dataUrlToBlob(dataUrl);
	const database = await openWallpaperDatabase();

	try {
		const transaction = database.transaction(STORE_NAME, 'readwrite');
		transaction.objectStore(STORE_NAME).put({
			id,
			blob,
			type: blob.type,
			createdAt: Date.now()
		} satisfies StoredLocalWallpaper);
		await waitForTransaction(transaction);
	} finally {
		database.close();
	}

	return toLocalWallpaperReference(id);
}

export async function resolveLocalWallpaperUrl(reference: string): Promise<string | null> {
	const id = getLocalWallpaperId(reference);
	if (!id) return null;

	const cachedUrl = localWallpaperUrlCache.get(id);
	if (cachedUrl) return cachedUrl;

	const database = await openWallpaperDatabase();

	try {
		const transaction = database.transaction(STORE_NAME, 'readonly');
		const request = transaction.objectStore(STORE_NAME).get(id) as IDBRequest<
			StoredLocalWallpaper | undefined
		>;
		const wallpaper = await requestToPromise(request);
		await waitForTransaction(transaction);

		if (!wallpaper) return null;

		const url = URL.createObjectURL(wallpaper.blob);
		localWallpaperUrlCache.set(id, url);
		return url;
	} finally {
		database.close();
	}
}

export async function deleteLocalWallpaper(reference: string | null): Promise<void> {
	if (!reference) return;

	const id = getLocalWallpaperId(reference);
	if (!id) return;

	revokeCachedLocalWallpaperUrl(id);

	const database = await openWallpaperDatabase();

	try {
		const transaction = database.transaction(STORE_NAME, 'readwrite');
		transaction.objectStore(STORE_NAME).delete(id);
		await waitForTransaction(transaction);
	} finally {
		database.close();
	}
}

function openWallpaperDatabase(): Promise<IDBDatabase> {
	if (typeof indexedDB === 'undefined') {
		throw new Error('Local wallpaper storage is not available in this browser.');
	}

	return new Promise((resolve, reject) => {
		const request = indexedDB.open(DATABASE_NAME, DATABASE_VERSION);

		request.onupgradeneeded = () => {
			const database = request.result;
			if (!database.objectStoreNames.contains(STORE_NAME)) {
				database.createObjectStore(STORE_NAME, { keyPath: 'id' });
			}
		};
		request.onsuccess = () => resolve(request.result);
		request.onerror = () =>
			reject(request.error ?? new Error('Unable to open local wallpaper storage.'));
		request.onblocked = () =>
			reject(new Error('Local wallpaper storage is blocked by another browser tab.'));
	});
}

function requestToPromise<T>(request: IDBRequest<T>): Promise<T> {
	return new Promise((resolve, reject) => {
		request.onsuccess = () => resolve(request.result);
		request.onerror = () => reject(request.error ?? new Error('Local wallpaper storage failed.'));
	});
}

function waitForTransaction(transaction: IDBTransaction): Promise<void> {
	return new Promise((resolve, reject) => {
		transaction.oncomplete = () => resolve();
		transaction.onabort = () =>
			reject(transaction.error ?? new Error('Local wallpaper storage was aborted.'));
		transaction.onerror = () =>
			reject(transaction.error ?? new Error('Local wallpaper storage failed.'));
	});
}

function dataUrlToBlob(dataUrl: string): Blob {
	const commaIndex = dataUrl.indexOf(',');
	if (commaIndex === -1) throw new Error('Invalid image data URL.');

	const metadata = dataUrl.slice(0, commaIndex);
	const encoded = dataUrl.slice(commaIndex + 1);
	const mimeType = metadata.match(/^data:([^;]+);base64$/i)?.[1];
	if (!mimeType?.startsWith('image/')) throw new Error('Invalid image data URL.');

	const binary = atob(encoded);
	const bytes = new Uint8Array(binary.length);
	for (let index = 0; index < binary.length; index += 1) {
		bytes[index] = binary.charCodeAt(index);
	}

	return new Blob([bytes], { type: mimeType });
}

function createLocalWallpaperId(): string {
	if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
		return crypto.randomUUID();
	}

	return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}

function revokeCachedLocalWallpaperUrl(id: string): void {
	const cachedUrl = localWallpaperUrlCache.get(id);
	if (!cachedUrl) return;

	URL.revokeObjectURL(cachedUrl);
	localWallpaperUrlCache.delete(id);
}
