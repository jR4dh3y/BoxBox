import { SvelteSet } from 'svelte/reactivity';
import { api, ApiRequestError } from '#lib/api/client.js';
import { array, boolean, record, string } from '#lib/api/parse.js';
import { listRoots, type FileInfo, type MountPoint } from '#lib/api/files.js';
import { parseDirectory } from '#lib/api/directory.js';
import { sortFiles } from '#lib/browse/sortFiles.js';
import type { SortDir, SortField } from '#lib/types/files.js';
import type { DesktopVolume } from '#lib/api/desktopVolumes.js';
import type { ChooserServers } from './chooserServers.svelte.js';
import { isRemotePath, parseRemotePath } from '#lib/api/remotePaths.js';
import { VolumesController } from './volumes.svelte.js';

function integer(value: unknown): number {
	if (typeof value !== 'number' || !Number.isInteger(value))
		throw new Error('Invalid chooser number');
	return value;
}
function parseRequest(value: unknown) {
	const r = record(value, 'file chooser');
	const kind = string(r.kind, 'chooser mode');
	if (kind !== 'open' && kind !== 'save' && kind !== 'save-files')
		throw new Error('Invalid chooser mode');
	return {
		kind,
		title: string(r.title, 'title'),
		appId: string(r.appId, 'application'),
		acceptLabel: string(r.acceptLabel, 'accept label').replaceAll('_', ''),
		multiple: boolean(r.multiple, 'multiple'),
		directory: boolean(r.directory, 'directory'),
		folder: string(r.folder, 'folder'),
		name: string(r.name, 'filename'),
		files: array(r.files, 'filenames').map((v) => string(v, 'filename')),
		filter: integer(r.filter),
		filters: array(r.filters, 'filters').map((value) => {
			const f = record(value, 'filter');
			return {
				name: string(f.name, 'filter name'),
				rules: array(f.rules, 'filter rules').map((value) => {
					const rule = record(value, 'filter rule');
					const kind = integer(rule.kind);
					if (kind !== 0 && kind !== 1) throw new Error('Invalid filter kind');
					return { kind, value: string(rule.value, 'filter pattern') };
				})
			};
		}),
		choices: array(r.choices, 'choices').map((value) => {
			const c = record(value, 'choice');
			return {
				id: string(c.id, 'choice id'),
				label: string(c.label, 'choice label'),
				value: string(c.value, 'choice value'),
				options: array(c.options ?? [], 'choice options').map((value) => {
					const o = record(value, 'choice option');
					return { id: string(o.id, 'option id'), label: string(o.label, 'option label') };
				})
			};
		})
	};
}

/** One portal picker. Directory reads and sort choices use the same contracts as Browse. */
export class FileChooser {
	servers = $state<ChooserServers | null>(null);
	serversError = $state('');
	#background = new AbortController();
	view = $state<'files' | 'locations'>('files');
	request = $state<ReturnType<typeof parseRequest> | null>(null);
	roots = $state<MountPoint[]>([]);
	path = $state('');
	location = $state('');
	query = $state('');
	name = $state('');
	items = $state<FileInfo[]>([]);
	selected = $state(new SvelteSet<string>());
	hidden = $state(false);
	filter = $state(0);
	choices = $state<Record<string, string>>({});
	sortBy = $state<SortField>('modTime');
	sortDir = $state<SortDir>('desc');
	loading = $state(false);
	busy = $state(false);
	error = $state('');
	replace = $state(false);
	done = $state(false);
	#abort: AbortController | undefined;
	#replacementTarget: string | undefined;
	constructor(
		readonly id: string,
		readonly volumes = new VolumesController()
	) {}
	get visible() {
		const filter = this.request?.filters[this.filter];
		const query = this.query.trim().toLocaleLowerCase();
		return sortFiles(
			this.items.filter((item) => {
				if (!item.name.toLocaleLowerCase().includes(query)) return false;
				if (!this.hidden && item.name.startsWith('.')) return false;
				if (item.isDir) return true;
				if (this.request?.directory) return false;
				return (
					!filter ||
					filter.rules.length === 0 ||
					filter.rules.some((rule) => {
						if (rule.kind === 1)
							return (
								item.mimeType?.split(';')[0] === rule.value ||
								(rule.value.endsWith('/*') && item.mimeType?.startsWith(rule.value.slice(0, -1)))
							);
						return globMatch(rule.value, item.name);
					})
				);
			}),
			this.sortBy,
			this.sortDir
		);
	}
	get remote() {
		return isRemotePath(this.path);
	}
	get remoteUnavailable() {
		return this.remote && (this.request?.kind !== 'open' || this.request.directory);
	}
	async initialize() {
		try {
			const [request, roots] = await Promise.all([
				api.get<unknown>(`/desktop/chooser/${this.id}`),
				listRoots()
			]);
			this.request = parseRequest(request);
			this.roots = roots.roots.filter((root) => !root.name.startsWith('@'));
			this.name = this.request.name;
			this.filter = this.request.filter;
			this.choices = Object.fromEntries(
				this.request.choices.map((choice) => [choice.id, choice.value])
			);
			await this.open(this.request.folder || this.roots[0]?.name || '');
			// Start network discovery after the local picker has had a chance to paint.
			requestAnimationFrame(() =>
				requestAnimationFrame(() => {
					void this.loadServers();
				})
			);
		} catch (error) {
			this.fail(error);
		}
	}
	async loadServers() {
		this.serversError = '';
		if (this.#background.signal.aborted) return;
		try {
			const { ChooserServers } = await import('./chooserServers.svelte.js');
			if (this.#background.signal.aborted) return;
			this.servers ??= new ChooserServers();
			await this.servers.load(this.#background.signal);
		} catch {
			if (!this.#background.signal.aborted)
				this.serversError = 'Server locations could not be loaded.';
		}
	}
	async open(path: string) {
		this.view = 'files';
		this.#abort?.abort();
		const abort = new AbortController();
		this.#abort = abort;
		this.loading = true;
		this.error = '';
		this.replace = false;
		this.selected = new SvelteSet();
		try {
			const response = parseDirectory(
				await api.get<unknown>(`/desktop/chooser/${this.id}/directory`, { path }, abort.signal)
			);
			if (abort.signal.aborted) return;
			this.items = response.items;
			this.path = path;
			this.location = path;
			this.query = '';
		} catch (error) {
			if (!abort.signal.aborted) this.fail(error);
		} finally {
			if (this.#abort === abort) this.loading = false;
		}
	}
	parent() {
		const source = parseRemotePath(this.path);
		if (source.kind === 'remote' && !source.path.includes('/')) this.showLocations();
		else void this.open(this.path.slice(0, this.path.lastIndexOf('/')));
	}
	showLocations() {
		this.#abort?.abort();
		this.loading = false;
		this.view = 'locations';
		this.selected = new SvelteSet();
		this.query = '';
		this.replace = false;
		this.error = '';
	}
	async openVolume(volume: DesktopVolume) {
		if (this.volumes.busy) return;
		let current = volume;
		if (!volume.mounted) {
			if (!volume.canMount || !(await this.volumes.act(volume, 'mount'))) return;
			const mounted = this.volumes.snapshot.value?.volumes.find((item) => item.id === volume.id);
			if (!mounted || this.view !== 'locations') return;
			current = mounted;
		}
		if (current.browsePath) await this.open(current.browsePath);
		else this.error = 'This drive does not expose a local folder that BoxBox can select from.';
	}
	search(query: string) {
		this.query = query;
		const visible = new SvelteSet(this.visible.map((item) => item.path));
		this.selected = new SvelteSet([...this.selected].filter((path) => visible.has(path)));
		this.replace = false;
	}
	choose(item: FileInfo) {
		if (item.isDir) {
			void this.open(item.path);
			return;
		}
		if (this.request?.kind === 'save') this.name = item.name;
		else {
			this.selected = new SvelteSet([item.path]);
			if (!this.request?.multiple) void this.accept();
		}
	}
	select(paths: Set<string>) {
		const eligible = this.visible.filter(
			(item) => paths.has(item.path) && item.isDir === this.request?.directory
		);
		this.selected = new SvelteSet(
			(this.request?.multiple ? eligible : eligible.slice(-1)).map((item) => item.path)
		);
		if (this.request?.kind === 'save' && eligible[0]) this.name = eligible[0].name;
		this.replace = false;
	}
	async accept() {
		if (this.busy || !this.request || this.view !== 'files' || this.remoteUnavailable) return;
		let paths: string[];
		if (this.request.kind === 'save') {
			if (!this.name || this.name === '.' || this.name === '..' || /[/\0]/.test(this.name)) {
				this.error = 'Enter a file name without a slash.';
				return;
			}
			paths = [`${this.path}/${this.name}`];
		} else if (this.request.kind === 'save-files') paths = [this.path];
		else
			paths = this.selected.size ? [...this.selected] : this.request.directory ? [this.path] : [];
		if (!paths.length) return;
		this.busy = true;
		this.error = '';
		try {
			await api.post(`/desktop/chooser/${this.id}`, {
				paths,
				filter: this.filter,
				choices: this.choices,
				replace: this.replace && this.#replacementTarget === JSON.stringify(paths)
			});
			this.done = true;
		} catch (error) {
			if (error instanceof ApiRequestError && error.status === 409) {
				this.replace = true;
				this.#replacementTarget = JSON.stringify(paths);
				this.error = 'A file with this name already exists. Choose Replace to overwrite it.';
			} else this.fail(error);
		} finally {
			this.busy = false;
		}
	}
	async cancel() {
		try {
			await api.post(`/desktop/chooser/${this.id}`, { cancel: true });
			this.done = true;
		} catch (error) {
			this.fail(error);
		}
	}
	dispose() {
		this.#abort?.abort();
		this.#background.abort();
	}
	fail(error: unknown) {
		this.error = error instanceof Error ? error.message : 'Unable to open the file chooser.';
	}
}

// Portal glob syntax is shell-style; character classes stay intact, regex operators are escaped.
export function globMatch(pattern: string, name: string): boolean {
	let source = '';
	for (let i = 0; i < pattern.length; i++) {
		const char = pattern[i];
		if (char === '*') source += '.*';
		else if (char === '?') source += '.';
		else if (char === '[') {
			const end = pattern.indexOf(']', i + 1);
			if (end < 0) source += '\\[';
			else {
				source += pattern.slice(i, end + 1).replace('[!', '[^');
				i = end;
			}
		} else source += char?.replace(/[\\^$.*+?()[\]{}|]/g, '\\$&') ?? '';
	}
	try {
		return new RegExp(`^${source}$`, 'u').test(name);
	} catch {
		return false;
	}
}
