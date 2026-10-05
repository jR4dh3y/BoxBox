import { flushSync } from 'svelte';
import { websocketStore } from './websocket.svelte';

/** Calls `connect()` from an effect, as the root layout does. */
export function connectFromEffect(): () => void {
	return $effect.root(() => {
		$effect(() => {
			websocketStore.connect(true);
		});
	});
}

export { flushSync, websocketStore };
