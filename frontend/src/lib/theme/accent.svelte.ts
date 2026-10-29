export type AccentState =
	| { kind: 'manual' }
	| { kind: 'loading' }
	| { kind: 'ready'; color: string }
	| { kind: 'unavailable' };

class AccentTheme {
	current = $state.raw<AccentState>({ kind: 'manual' });
	wallpaperColor = $state<string | null>(null);
	private applied = false;

	/** Each effect owns its work; a replaced wallpaper or session cannot apply a late result. */
	apply(
		customColor: string | null,
		wallpaper: string | null,
		surfaceWallpaper: string | null = null
	): () => void {
		const controller = new AbortController();
		const { signal } = controller;
		this.current = wallpaper ? { kind: 'loading' } : { kind: 'manual' };
		this.wallpaperColor = null;
		if (wallpaper || surfaceWallpaper || customColor || this.applied) {
			void import('./applyAccent.js')
				.then(({ applyAccentTheme }) =>
					applyAccentTheme(
						customColor,
						wallpaper,
						signal,
						(state, applied, wallpaperColor) => {
							if (signal.aborted) return;
							this.applied = applied;
							this.current = state;
							this.wallpaperColor = wallpaperColor;
						},
						surfaceWallpaper
					)
				)
				.catch(() => {
					if (!signal.aborted) this.current = { kind: 'unavailable' };
				});
		}
		return () => controller.abort();
	}
}

export const accentTheme = new AccentTheme();
