/** Runs `run` only once the pointer rests on a key, so sweeping across a list prefetches nothing. */
export function createHoverIntent(run: (key: string) => void, delayMs: number) {
	let timer: ReturnType<typeof setTimeout> | undefined;

	return (key: string | null) => {
		clearTimeout(timer);
		if (key !== null) timer = setTimeout(() => run(key), delayMs);
	};
}
