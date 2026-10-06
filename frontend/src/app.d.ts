// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		interface PageState {
			/** Position in the browse history, so back and forward can be enabled. */
			browseHistoryIndex?: number;
		}
		// interface Platform {}
	}
}

export {};
