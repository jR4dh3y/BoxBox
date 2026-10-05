/**
 * The share owner's wallpaper. Share pages set it so the root layout draws the
 * owner's wallpaper instead of the visitor's own.
 */
import { writable } from 'svelte/store';
import type { BackgroundImageMode } from '$lib/utils/wallpaper';

export interface ShareWallpaper {
	url: string;
	mode: BackgroundImageMode;
	frostedGlass: boolean;
}

export const shareWallpaper = writable<ShareWallpaper | null>(null);
