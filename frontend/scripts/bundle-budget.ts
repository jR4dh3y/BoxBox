/**
 * Ratchet on the JavaScript the app downloads without a dynamic import: the entry points,
 * every route node and all their static imports. Lazy-loaded code is excluded, so moving code behind
 * `import()` lowers this number. The gate is raw bytes: gzip output differs between Bun versions, so a
 * compressed size measured here would not match CI. Gzip is printed for reference only.
 *
 *   bun scripts/bundle-budget.ts          fail if the build is over budget
 *   bun scripts/bundle-budget.ts --lower  lower the budget to the current size; never raises it
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';
import { findChunkCycles, type ManifestChunk } from './chunk-cycles';

const clientDir = join(import.meta.dir, '..', '.svelte-kit', 'output', 'client');
const budgetFile = join(import.meta.dir, '..', 'perf-budget.json');
// Builds of the same source differ by a few bytes, so the budget moves in whole KiB.
const ROUNDING_BYTES = 1024;

interface Budget {
	staticJsBytes: number;
}

function readManifest(): Record<string, ManifestChunk> {
	return JSON.parse(readFileSync(join(clientDir, '.vite', 'manifest.json'), 'utf8')) as Record<
		string,
		ManifestChunk
	>;
}

function staticScripts(manifest: Record<string, ManifestChunk>): string[] {
	const roots = Object.keys(manifest).filter(
		(key) => key.includes('/nodes/') || key.includes('/runtime/client/entry')
	);
	if (roots.length === 0)
		throw new Error('No entry or route nodes in the manifest; run `bun run build` first');

	const files = new Set<string>();
	const visit = (key: string) => {
		const chunk = manifest[key];
		if (!chunk || files.has(chunk.file)) return;
		files.add(chunk.file);
		chunk.imports?.forEach(visit);
	};
	roots.forEach(visit);
	// The entry that boots the app is not a manifest key of its own.
	for (const key of Object.keys(manifest))
		if (manifest[key]?.file.includes('/entry/app.')) visit(key);
	return [...files];
}

function measure(manifest: Record<string, ManifestChunk>): { raw: number; gzip: number } {
	let raw = 0;
	let gzip = 0;
	for (const file of staticScripts(manifest)) {
		const content = readFileSync(join(clientDir, file));
		raw += content.length;
		gzip += gzipSync(content).length;
	}
	return { raw, gzip };
}

const manifest = readManifest();
const { raw: current, gzip } = measure(manifest);
const budget = (JSON.parse(readFileSync(budgetFile, 'utf8')) as Budget).staticJsBytes;
console.log(`static JS: ${current} B raw (budget ${budget} B), ${gzip} B gzip for reference`);

const cycles = findChunkCycles(manifest);
if (cycles.length > 0) {
	console.error(
		`Chunk import loops (${cycles.length}), such as ${cycles[0].join(' -> ')}.\n` +
			'A loop can run a module before its chunk has initialised and leave a blank page. Fix the chunk groups in vite.config.ts.'
	);
	process.exit(1);
}

if (current > budget) {
	console.error(
		`Over budget by ${current - budget} B. Lazy-load the new code or make it smaller; do not raise the budget.`
	);
	process.exit(1);
}

if (process.argv.includes('--lower')) {
	const lowered = Math.ceil(current / ROUNDING_BYTES) * ROUNDING_BYTES;
	if (lowered < budget) {
		writeFileSync(budgetFile, `${JSON.stringify({ staticJsBytes: lowered }, null, '\t')}\n`);
		console.log(`budget lowered to ${lowered} B`);
	}
}
