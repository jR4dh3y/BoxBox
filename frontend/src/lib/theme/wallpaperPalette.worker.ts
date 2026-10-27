import { wallpaperPaletteFromPixels } from './wallpaperPalette.js';

self.onmessage = (event: MessageEvent<unknown>) => {
	if (!(event.data instanceof Uint8ClampedArray)) {
		self.postMessage(null);
		return;
	}
	self.postMessage(wallpaperPaletteFromPixels(event.data));
};
