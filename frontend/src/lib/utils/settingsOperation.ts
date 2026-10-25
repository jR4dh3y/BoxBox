import { settingsStore } from '#lib/stores/settings.svelte.js';

/** A page belongs to one settings session, including same-account sign-ins. */
export class SettingsOperations {
	private readonly session = settingsStore.sessionSignal;
	private readonly page = new AbortController();
	private operation = new AbortController();

	start(): AbortSignal {
		this.operation.abort();
		this.operation = new AbortController();
		return AbortSignal.any([this.session, this.page.signal, this.operation.signal]);
	}

	dispose(): void {
		this.page.abort();
	}
}
