import { parseWallpaperPalette, type WallpaperPalette } from './wallpaperPaletteProtocol.js';

function extractPalette(
	pixels: Uint8ClampedArray,
	signal: AbortSignal
): Promise<WallpaperPalette | null> {
	signal.throwIfAborted();
	const worker = new Worker(new URL('./wallpaperPalette.worker.ts', import.meta.url), {
		type: 'module'
	});
	return new Promise((resolve, reject) => {
		function finish() {
			signal.removeEventListener('abort', cancel);
			worker.terminate();
		}
		function cancel() {
			finish();
			reject(new DOMException('Wallpaper replaced', 'AbortError'));
		}
		signal.addEventListener('abort', cancel, { once: true });
		worker.onmessage = (event: MessageEvent<unknown>) => {
			finish();
			resolve(parseWallpaperPalette(event.data));
		};
		worker.onerror = () => {
			finish();
			reject(new Error('Unable to extract wallpaper colors'));
		};
		worker.postMessage(pixels, [pixels.buffer]);
	});
}

export async function sampleWallpaperAccent(
	url: string,
	signal: AbortSignal
): Promise<WallpaperPalette | null> {
	signal.throwIfAborted();
	const image = new Image();
	image.crossOrigin = 'anonymous';
	image.decoding = 'async';
	const cancel = () => image.removeAttribute('src');
	signal.addEventListener('abort', cancel, { once: true });
	try {
		image.src = url;
		await image.decode();
		signal.throwIfAborted();
		// Only a small raster crosses to the worker; it never receives the full image.
		const canvas = document.createElement('canvas');
		canvas.width = canvas.height = 48;
		const context = canvas.getContext('2d', { willReadFrequently: true });
		if (!context) return null;
		context.drawImage(image, 0, 0, 48, 48);
		return await extractPalette(context.getImageData(0, 0, 48, 48).data, signal);
	} finally {
		signal.removeEventListener('abort', cancel);
		cancel();
	}
}
