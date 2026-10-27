import { settingsStore, type UserSettings } from '#lib/stores/settings.svelte.js';
import { SettingsOperations } from '#lib/utils/settingsOperation.js';
import {
	normalizeAccentColor,
	normalizeBackgroundImage,
	type AccentColorSource
} from '#lib/utils/appearance.js';
import { normalizeBackgroundImageMode, type BackgroundImageMode } from '#lib/utils/wallpaper.js';
import { publishShareWallpaper } from '#lib/utils/wallpaperSync.js';
import {
	deleteLocalWallpaper,
	isInlineWallpaperDataUrl,
	isLocalWallpaperReference,
	saveLocalWallpaperDataUrl
} from '#lib/utils/wallpaperStorage.js';

/** The message to show when a change could not be fully applied, or null when it was. */
export type PreferenceProblem = string | null;

function isStorageQuotaError(error: unknown): boolean {
	const errorText = error instanceof Error ? `${error.name} ${error.message}` : String(error);
	return /quota|NS_ERROR_DOM_QUOTA_REACHED|exceeded/i.test(errorText);
}

export function applyErrorMessage(error: unknown): string {
	if (isStorageQuotaError(error)) {
		return 'This wallpaper is too large to save locally. Choose a smaller image or pick one from the server.';
	}
	return error instanceof Error ? error.message : 'Unable to apply settings.';
}

function cleanupLocalWallpaper(previous: string | null, next: string | null): void {
	if (!previous || previous === next || !isLocalWallpaperReference(previous)) return;
	deleteLocalWallpaper(previous).catch(() => {
		// Cleanup failure should not make an already saved preference look failed.
	});
}

/**
 * Applies a preference the moment it changes. Anything that touches the wallpaper is saved
 * locally and published to the user's share pages, and a newer change cancels an older one.
 */
export class Preferences {
	private readonly operations = new SettingsOperations();

	dispose(): void {
		this.operations.dispose();
	}

	/** For settings that live only in the store. */
	update(patch: Partial<UserSettings>): void {
		settingsStore.set({ ...settingsStore.current, ...patch });
	}

	setAccentColor(color: string | null): void {
		this.update({ accentColor: normalizeAccentColor(color), accentColorSource: 'manual' });
	}

	setAccentColorSource(accentColorSource: AccentColorSource): void {
		this.update({ accentColorSource });
	}

	setWallpaper(
		backgroundImage: string,
		name: string,
		mode: BackgroundImageMode
	): Promise<PreferenceProblem> {
		return this.commit({
			...settingsStore.current,
			backgroundImage,
			backgroundImageName: name,
			backgroundImageMode: mode
		});
	}

	clearWallpaper(): Promise<PreferenceProblem> {
		return this.commit({
			...settingsStore.current,
			backgroundImage: null,
			backgroundImageName: null,
			frostedGlass: false
		});
	}

	setWallpaperMode(mode: BackgroundImageMode): Promise<PreferenceProblem> {
		return this.commit({ ...settingsStore.current, backgroundImageMode: mode });
	}

	setFrostedGlass(frostedGlass: boolean): Promise<PreferenceProblem> {
		return this.commit({ ...settingsStore.current, frostedGlass });
	}

	async reset(): Promise<PreferenceProblem> {
		const signal = this.operations.start();
		const previousBackgroundImage = settingsStore.current.backgroundImage;
		const publication = settingsStore.reset();
		cleanupLocalWallpaper(previousBackgroundImage, null);
		try {
			await publishShareWallpaper(publication, false, signal);
			return null;
		} catch (error) {
			if (signal.aborted) return null;
			return `Reset, but share pages still show the old wallpaper: ${
				error instanceof Error ? error.message : String(error)
			}`;
		}
	}

	private async commit(next: UserSettings): Promise<PreferenceProblem> {
		const signal = this.operations.start();
		const previous = settingsStore.current;
		let savedLocalImage: string | null = null;

		try {
			let backgroundImage = normalizeBackgroundImage(next.backgroundImage);
			if (backgroundImage && isInlineWallpaperDataUrl(backgroundImage)) {
				backgroundImage = await saveLocalWallpaperDataUrl(backgroundImage);
				savedLocalImage = backgroundImage;
				signal.throwIfAborted();
			}

			const applied: UserSettings = {
				...settingsStore.current,
				backgroundImage,
				backgroundImageName: backgroundImage ? next.backgroundImageName : null,
				backgroundImageMode: normalizeBackgroundImageMode(next.backgroundImageMode),
				frostedGlass: backgroundImage ? next.frostedGlass : false
			};

			signal.throwIfAborted();
			const publication = settingsStore.set(
				applied,
				previous.backgroundImage !== applied.backgroundImage
			);
			savedLocalImage = null;
			cleanupLocalWallpaper(previous.backgroundImage, applied.backgroundImage);

			try {
				await publishShareWallpaper(
					publication,
					previous.backgroundImageMode !== applied.backgroundImageMode ||
						previous.frostedGlass !== applied.frostedGlass,
					signal
				);
			} catch (error) {
				if (signal.aborted) return null;
				return `Saved, but share pages still show the old wallpaper: ${
					error instanceof Error ? error.message : String(error)
				}`;
			}
			return null;
		} catch (error) {
			cleanupLocalWallpaper(savedLocalImage, null);
			return signal.aborted ? null : applyErrorMessage(error);
		}
	}
}
