import assert from 'node:assert/strict';
import { afterEach, test } from 'node:test';
import type { UserSettings } from '#lib/stores/settings.svelte.js';
import { DesktopPreferences } from './desktopPreferences';

const realFetch = globalThis.fetch;
const realWindow = globalThis.window;
const hash = 'a'.repeat(64);

const defaults: UserSettings = {
	showHiddenFiles: false,
	showFileExtensions: true,
	confirmDelete: true,
	defaultSortBy: 'name',
	defaultSortDir: 'asc',
	defaultViewMode: 'list',
	accentColor: null,
	accentColorSource: 'wallpaper',
	backgroundImage: null,
	backgroundImageName: null,
	backgroundImageMode: 'cover',
	frostedGlass: false,
	wallpaperTint: false,
	hardwareAcceleration: true,
	previewOnSingleClick: false,
	compactMode: false,
	selectionCheckboxes: true,
	driveNameOverrides: {},
	favoriteFolders: []
};

/** Serves a saved preferences file and records every write the loader makes. */
function serve(saved: unknown): string[] {
	const writes: string[] = [];
	Object.assign(globalThis, {
		window: Object.assign(new EventTarget(), { location: { origin: 'http://desktop.test' } })
	});
	globalThis.fetch = Object.assign(
		async (input: Parameters<typeof fetch>[0], init?: Parameters<typeof fetch>[1]) => {
			const path = new URL(String(input), 'http://desktop.test').pathname;
			if (init?.method === 'PUT' && path === '/api/v1/desktop/preferences') {
				writes.push(String(init.body));
				return new Response(null, { status: 204 });
			}
			if (init?.method === 'PUT' && path === '/api/v1/desktop/wallpaper')
				return Response.json({ sha256: hash });
			return Response.json(saved);
		},
		{ preconnect: realFetch.preconnect }
	);
	return writes;
}

afterEach(() => {
	globalThis.fetch = realFetch;
	Object.assign(globalThis, { window: realWindow });
});

async function load(fallback: UserSettings): Promise<UserSettings> {
	const preferences = new DesktopPreferences(new AbortController().signal);
	const loaded = await preferences.load(defaults, fallback);
	await preferences.flush();
	return loaded;
}

test('an adopted fallback wallpaper is saved so the server keeps it', async () => {
	const writes = serve(null);
	const fallback = { ...defaults, backgroundImage: `/api/v1/desktop/wallpaper/${hash}` };
	assert.deepEqual(await load(fallback), fallback);
	assert.equal(writes.length, 1);
	assert.equal(JSON.parse(writes[0]).backgroundImage, fallback.backgroundImage);
});

test('a migrated wallpaper is saved with its staged reference', async () => {
	const writes = serve({ backgroundImage: 'data:image/png;base64,iVBORw0KGgo=' });
	const loaded = await load(defaults);
	assert.equal(loaded.backgroundImage, `/api/v1/desktop/wallpaper/${hash}`);
	assert.equal(writes.length, 1);
	assert.equal(JSON.parse(writes[0]).backgroundImage, loaded.backgroundImage);
});

test('an unchanged saved file is not rewritten', async () => {
	const writes = serve({ backgroundImage: `/api/v1/desktop/wallpaper/${hash}` });
	await load(defaults);
	assert.deepEqual(writes, []);
});

test('desktop rendering and tint choices survive a new settings session', async () => {
	serve({ hardwareAcceleration: false, wallpaperTint: true });
	const saved = await load(defaults);
	assert.equal(saved.hardwareAcceleration, false);
	assert.equal(saved.wallpaperTint, true);
});
