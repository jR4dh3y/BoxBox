// Writes the Lucide icons the web UI imports as SVG files, from the frontend's
// installed lucide-svelte, so both UIs draw identical glyphs.
// Run: bun run scripts/extract-icons.ts (after `bun install --cwd frontend`).
import { mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

const ICONS = [
	'ArrowUpFromLine', 'ChevronDown', 'ChevronLeft', 'ChevronRight', 'ChevronUp', 'Columns2',
	'Download', 'File', 'FileArchive', 'FileAudio', 'FileCode', 'FileImage', 'FileJson',
	'FileSpreadsheet', 'FileText', 'FileVideo', 'Folder', 'FolderOpen', 'Globe', 'HardDrive',
	'Home', 'Image', 'LayoutGrid', 'List', 'Monitor', 'Music', 'Palette', 'PanelLeft', 'Plus',
	'RefreshCw', 'Search', 'Server', 'Settings', 'Star', 'Trash2', 'Video', 'X',
	'AlertTriangle', 'AppWindow', 'CheckCircle', 'ClipboardPaste', 'Copy', 'ExternalLink', 'Info',
	'Check', 'Pencil', 'Scissors', 'XCircle', 'FilePlus', 'FolderPlus', 'Pin', 'PinOff', 'Share2', 'FolderInput',
	'Smartphone', 'Lock', 'PencilOff', 'Trash', 'Link', 'Send', 'Archive', 'Clock', 'Network', 'LayoutTemplate',
	'SquareTerminal', 'Play', 'Bookmark', 'LayoutDashboard',
	'FileTerminal', 'FileCog', 'FileMusic', 'FilePlay', 'FileType', 'FileKey', 'FileLock', 'FileBox',
	'Presentation', 'Disc3', 'Database', 'FileDiff', 'FolderDown', 'FolderSymlink', 'Heart'
];

const root = join(import.meta.dir, '../../frontend/node_modules/lucide-svelte/dist');
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

/** An icon's `iconNode` as SVG elements. */
function body(iconNode: string): string {
	return (JSON.parse(iconNode) as [string, Record<string, string>][])
		.map(([tag, attrs]) => {
			const list = Object.entries(attrs)
				.filter(([key]) => key !== 'key')
				.map(([key, value]) => `${key}="${value}"`)
				.join(' ');
			return `<${tag} ${list}/>`;
		})
		.join('');
}

const kebab = (name: string) =>
	name
		.replace(/([a-z])([A-Z0-9])/g, '$1-$2')
		.replace(/([A-Z])([A-Z][a-z])/g, '$1-$2')
		.replace(/([0-9])([A-Z])/g, '$1-$2')
		.toLowerCase();

for (const name of ICONS) {
	const pattern = new RegExp(`default as ${name} \\} from ['"](.+?)['"]`);
	const found = indexes
		.map(({ dir, text }) => ({ dir, match: pattern.exec(text) }))
		.find(({ match }) => match !== null);
	if (!found?.match) throw new Error(`lucide-svelte does not export ${name}`);
	const source = readFileSync(resolveSvelte(join(found.dir, found.match[1])), 'utf8');
	const node = /const iconNode = (\[.*\]);/.exec(source);
	if (!node) throw new Error(`no iconNode for ${name}`);
	const children = body(node[1]);
	// GPUI tints the SVG as a mask, so the stroke colour is irrelevant. The grid draws
	// file icons with strokeWidth 1.7, which needs its own file.
	for (const [dir, width] of [['', 2], ['thin', 1.7]] as const) {
		const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="${width}" stroke-linecap="round" stroke-linejoin="round">${children}</svg>\n`;
		mkdirSync(join(out, dir), { recursive: true });
		writeFileSync(join(out, dir, `${kebab(name)}.svg`), svg);
	}
}
// The folder outline with a gap at its bottom left, where an emblem stands (`components::emblemed`).
const emblemFolder = '<path d="M10.8 20H20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v6.2"/>';
for (const [dir, width] of [['', 2], ['thin', 1.7]] as const) {
	const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="${width}" stroke-linecap="round" stroke-linejoin="round">${emblemFolder}</svg>\n`;
	writeFileSync(join(out, dir, 'folder-emblem.svg'), svg);
}
console.log(`wrote ${ICONS.length} icons to ${out}`);

// Every Lucide icon as a folder emblem, one per line: name, the other names it has
// (aliases, for search), and its SVG body. The app builds each SVG at the stroke it
// needs (`emblems.rs`), so 1,600 icons cost one file instead of thousands.
const iconDir = join(root, 'icons');
const aliasWords = new Map<string, string[]>();
for (const match of readFileSync(join(root, 'aliases/aliases.js'), 'utf8').matchAll(/default as (\w+) \} from ['"](.+?)['"]/g)) {
	const target = /([^/]+)\.svelte$/.exec(resolveSvelte(join(root, 'aliases', match[2])))![1];
	aliasWords.set(target, [...(aliasWords.get(target) ?? []), kebab(match[1])]);
}
const lines = readdirSync(iconDir)
	.filter((file) => file.endsWith('.svelte'))
	.sort()
	.map((file) => {
		const name = file.slice(0, -'.svelte'.length);
		const node = /const iconNode = (\[.*\]);/.exec(readFileSync(join(iconDir, file), 'utf8'));
		if (!node) throw new Error(`no iconNode for ${name}`);
		return `${name}\t${(aliasWords.get(name) ?? []).join(' ')}\t${body(node[1])}`;
	});
writeFileSync(join(import.meta.dir, '../assets/emblems.tsv'), lines.join('\n') + '\n');
console.log(`wrote ${lines.length} emblems`);
