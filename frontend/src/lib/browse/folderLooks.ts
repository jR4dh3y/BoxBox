/**
 * How the user customised a folder (its menu → Customize Folder): a colour for the folder,
 * an emblem from every Lucide icon and the emblem's own colour, kept by browse path.
 */
import { isRecord } from '#lib/api/parse.js';

/** Which emblem a folder carries: a special folder's own, none, or a chosen icon. */
export type EmblemChoice = { kind: 'own' } | { kind: 'none' } | { kind: 'icon'; name: string };

export interface FolderLook {
	emblem: EmblemChoice;
	/** Null keeps the folder colour. */
	color: string | null;
	/** Null draws the emblem in the folder's colour. */
	emblemColor: string | null;
}

export type FolderLooks = Readonly<Record<string, FolderLook>>;

export const DEFAULT_LOOK: FolderLook = { emblem: { kind: 'own' }, color: null, emblemColor: null };

export function isDefaultLook(look: FolderLook): boolean {
	return look.emblem.kind === 'own' && look.color === null && look.emblemColor === null;
}

/** A default look is stored as nothing, so only what differs is kept. */
export function withFolderLook(looks: FolderLooks, path: string, look: FolderLook): FolderLooks {
	const rest = Object.fromEntries(Object.entries(looks).filter(([key]) => key !== path));
	return isDefaultLook(look) ? rest : { ...rest, [path]: look };
}

/** The emblems of special folders, by the name of the place they are. */
const SPECIAL_EMBLEMS: Readonly<Record<string, string>> = {
	home: 'house',
	desktop: 'monitor',
	documents: 'file-text',
	downloads: 'download',
	music: 'music',
	pictures: 'image',
	videos: 'video',
	templates: 'layout-template',
	public: 'share-2'
};

/**
 * The emblem a special folder carries: a place itself, or a folder of a place's name just
 * inside a drive, such as `home/Downloads`. The server does not say which real folder a place
 * is, so a folder of that name deeper down is an ordinary one.
 */
export function specialEmblem(path: string, places: ReadonlySet<string>): string | null {
	const parts = path.split('/');
	const name = parts.at(-1)?.toLowerCase() ?? '';
	const emblem = SPECIAL_EMBLEMS[name];
	if (!emblem) return null;
	if (parts.length === 1) return places.has(name) ? emblem : null;
	return parts.length === 2 && !path.startsWith('@remote/') ? emblem : null;
}

/** The emblem `look` gives a folder whose own is `own`. */
export function lookEmblem(look: FolderLook | undefined, own: string | null): string | null {
	const emblem = look?.emblem ?? DEFAULT_LOOK.emblem;
	if (emblem.kind === 'own') return own;
	return emblem.kind === 'icon' ? emblem.name : null;
}

function parseColor(value: unknown): string | null {
	return typeof value === 'string' && /^#[0-9a-f]{6}$/i.test(value) ? value : null;
}

function parseEmblem(value: unknown): EmblemChoice {
	if (!isRecord(value)) return DEFAULT_LOOK.emblem;
	if (value.kind === 'none') return { kind: 'none' };
	if (value.kind === 'icon' && typeof value.name === 'string' && /^[a-z0-9-]+$/.test(value.name))
		return { kind: 'icon', name: value.name };
	return DEFAULT_LOOK.emblem;
}

export function parseFolderLooks(value: unknown): FolderLooks {
	if (!isRecord(value)) return {};
	let looks: FolderLooks = {};
	for (const [path, saved] of Object.entries(value)) {
		if (!isRecord(saved)) continue;
		looks = withFolderLook(looks, path, {
			emblem: parseEmblem(saved.emblem),
			color: parseColor(saved.color),
			emblemColor: parseColor(saved.emblemColor)
		});
	}
	return looks;
}

export interface FolderAppearance {
	color: string | null;
	emblem: string | null;
	emblemColor: string | null;
}

/** How the folder at `path` is drawn. */
export function folderAppearance(
	path: string,
	places: ReadonlySet<string>,
	looks: FolderLooks
): FolderAppearance {
	const look = Object.hasOwn(looks, path) ? looks[path] : undefined;
	return {
		color: look?.color ?? null,
		emblem: lookEmblem(look, specialEmblem(path, places)),
		emblemColor: look?.emblemColor ?? null
	};
}
