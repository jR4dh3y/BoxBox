import assert from 'node:assert/strict';
import { test } from 'node:test';
import { setAccessToken } from '$lib/api/client';
import { authStore } from '$lib/stores/auth.svelte';
import { settingsStore } from '$lib/stores/settings.svelte';
import { resetShareWallpaperLoginSync, syncShareWallpaperOnLogin } from './wallpaperSync';

test('logout allows wallpaper synchronization once in each new session', async () => {
	const originalWindow = globalThis.window;
	const originalStorage = globalThis.localStorage;
	const originalSettings = settingsStore.current;
	const values = new Map<string, string>();
	let uploads = 0;
	const server = Bun.serve({
		port: 0,
		fetch(request) {
			if (request.method === 'PUT') uploads++;
			return request.method === 'GET'
				? Response.json({ error: 'Not found', code: 'NOT_FOUND' }, { status: 404 })
				: Response.json({ message: 'OK' });
		}
	});
	Object.assign(globalThis, {
		window: { location: { origin: server.url.origin } },
		localStorage: {
			getItem: (key: string) => values.get(key) ?? null,
			setItem: (key: string, value: string) => values.set(key, value),
			removeItem: (key: string) => values.delete(key)
		}
	});
	const signIn = (username: string) => {
		setAccessToken(`test.${btoa(JSON.stringify({ username }))}.test`);
	};
	try {
		resetShareWallpaperLoginSync();
		settingsStore.current = {
			...originalSettings,
			backgroundImage: 'data:image/png;base64,aGVsbG8='
		};
		signIn('alice');
		await syncShareWallpaperOnLogin();
		await syncShareWallpaperOnLogin();
		assert.equal(uploads, 1);

		await authStore.logout();
		signIn('alice');
		await syncShareWallpaperOnLogin();
		assert.equal(uploads, 2);

		await authStore.logout();
		signIn('bob');
		await syncShareWallpaperOnLogin();
		assert.equal(uploads, 2, 'another account must not inherit the browser wallpaper');

		await authStore.logout();
		signIn('alice');
		settingsStore.current = { ...originalSettings, backgroundImage: null };
		await syncShareWallpaperOnLogin();
		assert.equal(uploads, 2, 'an empty browser must not publish a wallpaper');
	} finally {
		await authStore.logout();
		settingsStore.current = originalSettings;
		Object.assign(globalThis, { window: originalWindow, localStorage: originalStorage });
		server.stop(true);
	}
});
