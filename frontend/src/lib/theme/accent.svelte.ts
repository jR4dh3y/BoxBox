export type AccentState =
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
			void import('./applyAccent.js')
				.then(({ applyAccentTheme }) =>
					applyAccentTheme(customColor, wallpaper, signal, (state, applied) => {
						if (signal.aborted) return;
						this.applied = applied;
						this.current = state;
					})
				)
				.catch(() => {
					if (!signal.aborted) this.current = { kind: 'unavailable' };
				});
		}
		return () => controller.abort();
	}
}

export const accentTheme = new AccentTheme();
