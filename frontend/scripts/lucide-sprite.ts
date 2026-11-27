import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import type { Plugin } from 'vite';

const prefix = '\0boxbox-lucide:';
const spritePath = '/boxbox-icons.svg';
const renderer = fileURLToPath(new URL('../src/lib/components/SpriteIcon.svelte', import.meta.url));
const spriteFile = fileURLToPath(
	new URL('../node_modules/lucide-static/sprite.svg', import.meta.url)
);
const licenseFile = fileURLToPath(
	new URL('../node_modules/lucide-static/LICENSE', import.meta.url)
);

/** The Lucide package's named imports keep their API and share one renderer and SVG asset. */
export function lucideSprite(): Plugin {
	const source = readFileSync(spriteFile, 'utf8');
	const symbols = new Map(
		Array.from(source.matchAll(/<symbol id="([^"]+)"[^>]*>[\s\S]*?<\/symbol>/g), ([symbol, id]) => [
			id,
			symbol
		])
	);
	const license = readFileSync(licenseFile, 'utf8');
	function sprite(names: Iterable<string>) {
		const selected = [...new Set(names)].sort().map((name) => symbols.get(name));
		return `<svg xmlns="http://www.w3.org/2000/svg"><!--${license.replaceAll('--', '—')}--><defs>${selected.join('')}</defs></svg>`;
	}
	return {
		name: 'boxbox-lucide-sprite',
		enforce: 'pre',
		resolveId(id, importer) {
			if (!importer?.includes('/lucide-svelte/dist/icons/')) return;
			const name = /^\.\/([a-z0-9-]+)\.svelte$/.exec(id)?.[1];
			if (name && symbols.has(name)) return prefix + name;
		},
		load(id) {
			if (!id.startsWith(prefix)) return;
			const name = id.slice(prefix.length);
			return `import Icon from ${JSON.stringify(renderer)};
				export default (anchor, props) => Icon(anchor, { name: ${JSON.stringify(name)}, props });`;
		},
		configureServer(server) {
			server.middlewares.use(spritePath, (_request, response) => {
				response.setHeader('Content-Type', 'image/svg+xml');
				response.end(sprite(symbols.keys()));
			});
		},
		renderChunk(code, _chunk, _options, { chunks }) {
			if (this.environment.name !== 'client' || !code.includes(spritePath)) return;
			const names = Object.values(chunks).flatMap((chunk) =>
				Object.keys(chunk.modules)
					.filter((id) => id.startsWith(prefix))
					.map((id) => id.slice(prefix.length))
			);
			const source = sprite(names);
			const hash = createHash('sha256').update(source).digest('hex').slice(0, 16);
			const fileName = `_app/immutable/assets/boxbox-icons-${hash}.svg`;
			this.emitFile({ type: 'asset', fileName, source });
			return { code: code.replaceAll(spritePath, `/${fileName}`), map: null };
		}
	};
}
