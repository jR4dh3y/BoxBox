/** One chunk of the Vite manifest: its file and the chunks it imports without `import()`. */
export interface ManifestChunk {
	file: string;
	imports?: string[];
}

/**
 * Loops in the static import graph, each as the files around the loop. ES modules run dependencies
 * first only while that graph has no loops. A loop between chunks can run a module before the chunk
 * it needs has initialised, which fails at startup and leaves a blank page.
 */
export function findChunkCycles(manifest: Record<string, ManifestChunk>): string[][] {
	const cycles: string[][] = [];
	const done = new Set<string>();
	const path: string[] = [];

	const visit = (key: string) => {
		const chunk = manifest[key];
		if (!chunk || done.has(key)) return;
		const loopStart = path.indexOf(key);
		if (loopStart >= 0) {
			cycles.push(
				[...path.slice(loopStart), key].map((member) => manifest[member]?.file ?? member)
			);
			return;
		}
		path.push(key);
		chunk.imports?.forEach(visit);
		path.pop();
		done.add(key);
	};
	Object.keys(manifest).forEach(visit);
	return cycles;
}
