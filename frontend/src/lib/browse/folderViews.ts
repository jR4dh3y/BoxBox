import { isRecord } from '#lib/api/parse.js';
import type { SortDir, SortField, ViewMode } from '#lib/types/files.js';

export interface FolderSort {
	by: SortField;
	dir: SortDir;
}

/** What one folder changed from Settings; a missing field follows the default. */
export interface FolderView {
	mode?: ViewMode;
	sort?: FolderSort;
}

/** Keyed by browse path, which already names the server for a remote folder. */
export type FolderViews = Readonly<Record<string, FolderView>>;

export interface FolderViewDefaults {
	mode: ViewMode;
	sort: FolderSort;
	/** Off: a view picked in any folder becomes every folder's view. */
	perFolder: boolean;
}

/** The root lists drives, not a folder, so it keeps its own view and no sort. */
export function isRootPath(path: string): boolean {
	return path === '' || /^@remote\/[^/]+$/.test(path);
}

/** The view and sort `path` opens with. */
export function folderView(
	views: FolderViews,
	path: string,
	defaults: FolderViewDefaults
): { mode: ViewMode; sort: FolderSort } {
	const saved = views[path];
	return {
		mode: (defaults.perFolder ? saved?.mode : undefined) ?? defaults.mode,
		sort: saved?.sort ?? defaults.sort
	};
}

function sameSort(left: FolderSort, right: FolderSort): boolean {
	return left.by === right.by && left.dir === right.dir;
}

function update(
	views: FolderViews,
	path: string,
	change: (view: FolderView) => FolderView
): FolderViews {
	const { [path]: current, ...rest } = views;
	const next = change(current ?? {});
	return next.mode === undefined && next.sort === undefined ? rest : { ...rest, [path]: next };
}

/** A default is stored as nothing, so only what differs is kept. */
export function rememberFolderMode(
	views: FolderViews,
	path: string,
	mode: ViewMode,
	defaultMode: ViewMode
): FolderViews {
	return update(views, path, ({ sort }) => (mode === defaultMode ? { sort } : { sort, mode }));
}

export function rememberFolderSort(
	views: FolderViews,
	path: string,
	sort: FolderSort,
	defaultSort: FolderSort
): FolderViews {
	return update(views, path, ({ mode }) =>
		sameSort(sort, defaultSort) ? { mode } : { mode, sort }
	);
}

/** Drops paths whose saved values no longer parse; the rest is kept as is. */
export function parseFolderViews(value: unknown): FolderViews {
	if (!isRecord(value)) return {};
	const views: Record<string, FolderView> = {};
	for (const [path, saved] of Object.entries(value)) {
		if (!isRecord(saved)) continue;
		const view: FolderView = {};
		if (saved.mode === 'list' || saved.mode === 'grid') view.mode = saved.mode;
		const sort = saved.sort;
		if (
			isRecord(sort) &&
			(sort.by === 'name' || sort.by === 'size' || sort.by === 'modTime' || sort.by === 'type') &&
			(sort.dir === 'asc' || sort.dir === 'desc')
		)
			view.sort = { by: sort.by, dir: sort.dir };
		if (view.mode !== undefined || view.sort !== undefined) views[path] = view;
	}
	return views;
}
