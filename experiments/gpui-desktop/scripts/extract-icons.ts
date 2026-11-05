// Writes the Lucide icons the web UI imports as SVG files, from the frontend's
// installed lucide-svelte, so both UIs draw identical glyphs.
// Run: bun run scripts/extract-icons.ts (after `bun install --cwd frontend`).
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

const ICONS = [
	'ArrowUpFromLine', 'ChevronDown', 'ChevronLeft', 'ChevronRight', 'ChevronUp', 'Columns2',
	'Download', 'File', 'FileArchive', 'FileAudio', 'FileCode', 'FileImage', 'FileJson',
	'FileSpreadsheet', 'FileText', 'FileVideo', 'Folder', 'FolderOpen', 'Globe', 'HardDrive',
	'Home', 'Image', 'LayoutGrid', 'List', 'Monitor', 'Music', 'Palette', 'PanelLeft', 'Plus',
	'RefreshCw', 'Search', 'Server', 'Settings', 'Star', 'Trash2', 'Video', 'X'
];

const root = join(import.meta.dir, '../../../frontend/node_modules/lucide-svelte/dist');
const indexes = ['icons/index.js', 'aliases/aliases.js'].map((file) => ({
	dir: dirname(join(root, file)),
	text: readFileSync(join(root, file), 'utf8')
}));
const out = join(import.meta.dir, '../assets/icons');
mkdirSync(out, { recursive: true });

function resolveSvelte(file: string): string {
	if (file.endsWith('.svelte')) return file;
	const next = /from ["'](.+?)["']/.exec(readFileSync(file, 'utf8'));
	if (!next) throw new Error(`no re-export in ${file}`);
	return resolveSvelte(join(dirname(file), next[1]));
}

const kebab = (name: string) =>
	name.replace(/([a-z])([A-Z0-9])/g, '$1-$2').replace(/([0-9])([A-Z])/g, '$1-$2').toLowerCase();

for (const name of ICONS) {
	const pattern = new RegExp(`default as ${name} \\} from ['"](.+?)['"]`);
	const found = indexes
		.map(({ dir, text }) => ({ dir, match: pattern.exec(text) }))
		.find(({ match }) => match !== null);
	if (!found?.match) throw new Error(`lucide-svelte does not export ${name}`);
	const source = readFileSync(resolveSvelte(join(found.dir, found.match[1])), 'utf8');
	const node = /const iconNode = (\[.*\]);/.exec(source);
	if (!node) throw new Error(`no iconNode for ${name}`);
	const children = (JSON.parse(node[1]) as [string, Record<string, string>][])
		.map(([tag, attrs]) => {
			const list = Object.entries(attrs)
				.filter(([key]) => key !== 'key')
				.map(([key, value]) => `${key}="${value}"`)
				.join(' ');
			return `<${tag} ${list}/>`;
		})
		.join('');
	// lucide defaults: 24 viewBox, 2px round strokes in currentColor.
	const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${children}</svg>\n`;
	writeFileSync(join(out, `${kebab(name)}.svg`), svg);
}
console.log(`wrote ${ICONS.length} icons to ${out}`);
