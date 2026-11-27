/**
 * Every Lucide icon, for folder emblems. The catalogue is large, so it loads only when a
 * folder shows a chosen emblem or the Customize dialog opens.
 */
import nodesUrl from 'lucide-static/icon-nodes.json?url';
import tagsUrl from 'lucide-static/tags.json?url';
import { isRecord } from '#lib/api/parse.js';

const SHAPES = ['path', 'circle', 'ellipse', 'line', 'polygon', 'polyline', 'rect'] as const;
type Shape = (typeof SHAPES)[number];

/** One shape of an icon, drawn in Lucide's 24-unit box. */
export interface EmblemShape {
	tag: Shape;
	attrs: Readonly<Record<string, string>>;
}

export interface EmblemCatalog {
	/** Icon names, in name order. */
	names: readonly string[];
	shapes(name: string): readonly EmblemShape[] | null;
	/** Names whose name or tags hold every word of `query`. */
	search(query: string): string[];
}

/** Offered first, before the rest in name order. */
export const COMMON_EMBLEMS = [
	'code',
	'terminal',
	'gamepad-2',
	'briefcase',
	'book',
	'graduation-cap',
	'file-text',
	'download',
	'image',
	'camera',
	'music',
	'headphones',
	'video',
	'film',
	'house',
	'monitor',
	'layout-template',
	'share-2',
	'cloud',
	'globe',
	'mail',
	'users',
	'heart',
	'star',
	'sparkles',
	'bot',
	'cpu',
	'database',
	'package',
	'box',
	'archive',
	'wrench',
	'flask-conical',
	'palette',
	'wallet',
	'lock',
	'git-branch',
	'server',
	'notebook-pen'
] as const;

function isShape(value: unknown): value is Shape {
	return SHAPES.some((shape) => shape === value);
}

/** Lucide's `icon-nodes.json`: each name's `[tag, attributes]` pairs. Unknown shapes drop the icon. */
export function parseIconNodes(value: unknown): Record<string, EmblemShape[]> {
	if (!isRecord(value)) throw new Error('Invalid icon catalogue');
	const icons: Record<string, EmblemShape[]> = {};
	for (const [name, node] of Object.entries(value)) {
		if (!Array.isArray(node)) continue;
		const shapes: EmblemShape[] = [];
		for (const entry of node) {
			if (!Array.isArray(entry) || !isShape(entry[0]) || !isRecord(entry[1])) break;
			const attrs: Record<string, string> = {};
			for (const [key, attr] of Object.entries(entry[1])) {
				// Geometry only: these attributes are spread onto SVG elements.
				if (/^on/i.test(key) || key === 'href' || key === 'style') continue;
				if (typeof attr === 'string' || typeof attr === 'number') attrs[key] = String(attr);
			}
			shapes.push({ tag: entry[0], attrs });
		}
		if (shapes.length === node.length) icons[name] = shapes;
	}
	return icons;
}

/** Lucide's `tags.json`: each name's search words. */
export function parseIconTags(value: unknown): Record<string, string[]> {
	if (!isRecord(value)) return {};
	const tags: Record<string, string[]> = {};
	for (const [name, words] of Object.entries(value)) {
		if (Array.isArray(words))
			tags[name] = words.filter((word): word is string => typeof word === 'string');
	}
	return tags;
}

export function createCatalog(
	nodes: Readonly<Record<string, readonly EmblemShape[]>>,
	tags: Readonly<Record<string, readonly string[]>>
): EmblemCatalog {
	const names = Object.keys(nodes).sort();
	const words = new Map(names.map((name) => [name, [name, ...(tags[name] ?? [])].join(' ')]));
	return {
		names,
		shapes: (name) => (Object.hasOwn(nodes, name) ? (nodes[name] ?? null) : null),
		search(query) {
			const wanted = query.toLowerCase().split(/\s+/).filter(Boolean);
			return names.filter((name) => {
				const text = words.get(name) ?? name;
				return wanted.every((word) => text.includes(word));
			});
		}
	};
}

let catalog: Promise<EmblemCatalog> | null = null;

async function fetchJson(url: string): Promise<unknown> {
	const response = await fetch(url);
	if (!response.ok) throw new Error(`Icons could not be loaded (${response.status})`);
	return response.json();
}

export function loadEmblems(): Promise<EmblemCatalog> {
	catalog ??= Promise.all([fetchJson(nodesUrl), fetchJson(tagsUrl)]).then(([nodes, tags]) =>
		createCatalog(parseIconNodes(nodes), parseIconTags(tags))
	);
	// A failed load is asked again next time, instead of failing for the session.
	catalog.catch(() => {
		catalog = null;
	});
	return catalog;
}
