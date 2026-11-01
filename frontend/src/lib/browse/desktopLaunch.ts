import { SvelteSet } from 'svelte/reactivity';
import type { BrowseTabsController } from './tabs.svelte';
import { parseRemotePath } from '#lib/api/remotePaths.js';

export interface DesktopLaunchTarget {
	path: string;
	select?: string;
}

interface LaunchSource extends EventTarget {
	boxboxDesktopTargets?: unknown;
}

declare global {
	interface Window {
		boxboxDesktopTargets?: unknown;
	}
}

export function parseDesktopLaunch(value: unknown): DesktopLaunchTarget | null {
	if (
		typeof value !== 'object' ||
		value === null ||
		!('path' in value) ||
		typeof value.path !== 'string'
	)
		return null;
	const path = value.path;
	if (
		!path ||
		path.length > 8192 ||
		path.includes('\\') ||
		Array.from(path).some(
			(character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127
		) ||
		path.split('/').some((part) => !part || part === '.' || part === '..')
	)
		return null;
	try {
		parseRemotePath(path);
	} catch {
		return null;
	}
	if ('select' in value && value.select !== undefined) {
		if (
			typeof value.select !== 'string' ||
			!value.select.startsWith(path + '/') ||
			value.select.slice(path.length + 1).includes('/') ||
			!parseDesktopLaunch({ path: value.select })
		)
			return null;
		return { path, select: value.select };
	}
	return { path };
}

/** Launches arriving before mount remain queued; only parsed host targets reach the workspace. */
export function subscribeDesktopLaunches(
	source: LaunchSource,
	open: (target: DesktopLaunchTarget) => void
): () => void {
	const drain = () => {
		const pending = source.boxboxDesktopTargets;
		source.boxboxDesktopTargets = [];
		if (!Array.isArray(pending)) return;
		const targets: unknown[] = pending;
		for (const value of targets) {
			const target = parseDesktopLaunch(value);
			if (target) open(target);
		}
	};
	source.addEventListener('boxbox-desktop-open', drain);
	drain();
	return () => source.removeEventListener('boxbox-desktop-open', drain);
}

export function openDesktopTab(tabs: BrowseTabsController, target: DesktopLaunchTarget): void {
	const snapshot = tabs.activePane?.location.snapshot;
	if (!snapshot) return;
	tabs.newTab({ ...snapshot, page: 'files', path: target.path, searchQuery: '' });
	if (target.select) tabs.activePane?.selection.set(new SvelteSet([target.select]));
}
