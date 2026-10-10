/**
 * Turns on `delayMs` after `source` does and off as soon as it does,
 * so a load that finishes quickly never flashes a spinner. Call during component setup.
 */
export function createDelayedFlag(source: () => boolean, delayMs: number) {
	let delayed = $state(false);

	$effect(() => {
		if (!source()) {
			delayed = false;
			return;
		}
		const timer = setTimeout(() => (delayed = true), delayMs);
		return () => clearTimeout(timer);
	});

	return {
		get value() {
			return delayed;
		}
	};
}
