type AccentState =
	| { kind: 'manual' }
	| { kind: 'loading' }
	| { kind: 'ready'; color: string }
	| { kind: 'unavailable' };

class AccentTheme {
	current = $state.raw<AccentState>({ kind: 'manual' });
	private applied = false;

	/** Each effect owns its work; a replaced wallpaper or session cannot apply a late result. */
	apply(customColor: string | null, wallpaper: string | null): () => void {
		const controller = new AbortController();
		const { signal } = controller;
		this.current = wallpaper ? { kind: 'loading' } : { kind: 'manual' };
		if (wallpaper || customColor || this.applied) {
			void import('./colors.js')
				.then(async ({ applyAccentColor }) => {
					if (signal.aborted) return;
					if (!wallpaper) {
						applyAccentColor(customColor);
						this.applied = customColor !== null;
						return;
					}
					let color: string | null = null;
					try {
						const { sampleWallpaperAccent } = await import('./wallpaperAccent.js');
						signal.throwIfAborted();
						color = await sampleWallpaperAccent(wallpaper, signal);
					} catch {
						signal.throwIfAborted();
					}
					if (signal.aborted) return;
					applyAccentColor(color ?? customColor);
					this.applied = color !== null || customColor !== null;
					this.current = color ? { kind: 'ready', color } : { kind: 'unavailable' };
				})
				.catch(() => {
					if (!signal.aborted) this.current = { kind: 'unavailable' };
				});
		}
		return () => controller.abort();
	}
}

export const accentTheme = new AccentTheme();
