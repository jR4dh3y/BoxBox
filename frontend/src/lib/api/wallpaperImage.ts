/** Image transport and mutations, loaded only for wallpaper restoration or publication. */
import { apiFetch, apiRequest, parseResponse } from './client';
import type { WallpaperDisplay } from './wallpaper';

export async function downloadWallpaper(signal?: AbortSignal): Promise<Blob> {
	const response = await apiFetch('/settings/wallpaper/image', { signal });
	if (!response.ok) await parseResponse<never>(response);
	if (!response.headers.get('Content-Type')?.startsWith('image/')) {
		throw new Error('Invalid wallpaper image');
	}
	return response.blob();
}

/** Upload a new wallpaper image with how it should be drawn and where it came from. */
export async function uploadWallpaper(
	image: Blob,
	display: WallpaperDisplay,
	source: string,
	signal?: AbortSignal
): Promise<void> {
	await apiRequest('/settings/wallpaper', {
		method: 'PUT',
		body: image,
		signal,
		params: { mode: display.mode, frostedGlass: String(display.frostedGlass), source }
	});
}

/** Change how the stored wallpaper is drawn without re-uploading it. */
export async function updateWallpaperDisplay(
	display: WallpaperDisplay,
	signal?: AbortSignal
): Promise<void> {
	await apiRequest('/settings/wallpaper', { method: 'PATCH', body: display, signal });
}

/** Remove the stored wallpaper. */
export async function deleteWallpaper(signal?: AbortSignal): Promise<void> {
	await apiRequest('/settings/wallpaper', { method: 'DELETE', signal });
}
