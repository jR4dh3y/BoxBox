import assert from 'node:assert/strict';
import { test } from 'node:test';
import { IDBFactory } from 'fake-indexeddb';
import { normalizeBackgroundImageMode } from './wallpaper';
import { STORAGE_KEYS } from '#lib/config.js';
import { ApiRequestError, clearTokens } from '#lib/api/client.js';
import { downloadWallpaper } from '#lib/api/wallpaperImage.js';
import { getWallpaper, type StoredWallpaper } from '#lib/api/wallpaper.js';
import { authStore } from '#lib/stores/auth.svelte.js';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import { wallpaperPublication } from './wallpaperPublication';
import { refreshWallpaperDraft } from './wallpaperDraft';
import {
	getLocalWallpaperBlob,
	saveLocalWallpaperBlob,
	toLocalWallpaperReference
} from './wallpaperStorage';
import {
	publishShareWallpaper,
	resetShareWallpaperLoginSync,
	syncShareWallpaperOnLogin
} from './wallpaperSync';

function publishEditedWallpaper() {
	const publication = settingsStore.set(settingsStore.current, true);
	return publishShareWallpaper(publication);
}

function deferred() {
	let resolve = () => {};
	const promise = new Promise<void>((done) => (resolve = done));
	return { promise, resolve };
}

async function withSession(run: (context: ReturnType<typeof createSession>) => Promise<void>) {
	const context = createSession();
	try {
		await run(context);
	} finally {
		await context.close();
	}
}

function createSession() {
	const original = {
		window: globalThis.window,
		document: globalThis.document,
		localStorage: globalThis.localStorage,
		indexedDB: globalThis.indexedDB
	};
	const originalSettings = settingsStore.current;
	const values = new Map<string, string>();
	const images = new Map<string, Blob>();
	const stored = new Map<string, StoredWallpaper>();
	const requests: string[] = [];
	const state = {
		beforeMetadata: async () => {},
		beforeImage: async () => {},
		beforeDriveNames: async (username: string) => {
			void username;
		},
		imageStatus: 200,
		uploadStatus: 200,
		invalidMetadata: false
	};
	const server = Bun.serve({
		port: 0,
		async fetch(request) {
			const path = new URL(request.url).pathname;
			requests.push(`${request.method} ${path}`);
			if (path.endsWith('/auth/login')) {
				const body: unknown = await request.json();
				assert.ok(typeof body === 'object' && body !== null && 'username' in body);
				return Response.json({
					accessToken: `test.${btoa(JSON.stringify({ username: body.username }))}.test`,
					expiresAt: '2030-01-01T00:00:00Z'
				});
			}
			if (path.endsWith('/auth/logout')) return Response.json({ message: 'Logged out' });
			const token = request.headers.get('Authorization')?.split('.')[1];
			const claims: unknown = JSON.parse(atob(token ?? 'e30='));
			assert.ok(
				typeof claims === 'object' &&
					claims !== null &&
					'username' in claims &&
					typeof claims.username === 'string'
			);
			const username = claims.username;
			if (path.endsWith('/drive-names')) {
				await state.beforeDriveNames(username);
				return Response.json({ mappings: [{ mountPoint: 'media', customName: username }] });
			}
			if (path.endsWith('/wallpaper/image')) {
				await state.beforeImage();
				if (state.imageStatus !== 200)
					return Response.json(
						{ error: 'Unavailable', code: 'INTERNAL_ERROR' },
						{ status: state.imageStatus }
					);
				return new Response(images.get(username), { headers: { 'Content-Type': 'image/png' } });
			}
			if (request.method === 'GET') {
				await state.beforeMetadata();
				if (state.invalidMetadata) return Response.json({ mode: 'invalid' });
				const metadata = stored.get(username);
				return metadata
					? Response.json(metadata)
					: Response.json({ error: 'Not found', code: 'NOT_FOUND' }, { status: 404 });
			}
			if (request.method === 'PUT') {
				if (state.uploadStatus !== 200)
					return Response.json(
						{ error: 'Unavailable', code: 'INTERNAL_ERROR' },
						{ status: state.uploadStatus }
					);
				const image = await request.blob();
				images.set(username, image);
				const params = new URL(request.url).searchParams;
				stored.set(username, {
					mode: normalizeBackgroundImageMode(params.get('mode')),
					frostedGlass: params.get('frostedGlass') === 'true',
					source: params.get('source') ?? '',
					sha256: new Bun.CryptoHasher('sha256').update(await image.arrayBuffer()).digest('hex')
				});
			}
			if (request.method === 'DELETE') stored.delete(username);
			return Response.json({ message: 'OK' });
		}
	});
	Object.assign(globalThis, {
		window: Object.assign(new EventTarget(), { location: { origin: server.url.origin } }),
		document: { querySelector: () => null },
		indexedDB: new IDBFactory(),
		localStorage: {
			getItem: (key: string) => values.get(key) ?? null,
			setItem: (key: string, value: string) => values.set(key, value),
			removeItem: (key: string) => values.delete(key)
		}
	});
	resetShareWallpaperLoginSync();
	settingsStore.clearSession();
	clearTokens();
	return {
		requests,
		values,
		stored,
		state,
		setWallpaper(
			username: string,
			contents = 'wallpaper',
			mode: StoredWallpaper['mode'] = 'contain'
		) {
			images.set(username, new Blob([contents], { type: 'image/png' }));
			const sha256 = new Bun.CryptoHasher('sha256').update(contents).digest('hex');
			stored.set(username, {
				mode,
				frostedGlass: true,
				source: 'another-device-reference',
				sha256
			});
			return toLocalWallpaperReference(sha256);
		},
		async login(username: string) {
			assert.equal(await authStore.login(username, 'password'), true);
		},
		async close() {
			await authStore.logout();
			settingsStore.current = originalSettings;
			Object.assign(globalThis, original);
			server.stop(true);
		}
	};
}

test('fresh login restores image bytes and display settings; later logins validate the cache', async () => {
	await withSession(async ({ setWallpaper, login, requests }) => {
		const reference = setWallpaper('alice');
		await login('alice');
		assert.equal(settingsStore.current.backgroundImage, null);
		await syncShareWallpaperOnLogin();
		await syncShareWallpaperOnLogin();
		assert.equal(settingsStore.current.backgroundImage, reference);
		assert.equal(settingsStore.current.backgroundImageMode, 'contain');
		assert.equal(settingsStore.current.frostedGlass, true);
		assert.equal(await (await getLocalWallpaperBlob(reference))?.text(), 'wallpaper');
		assert.equal(
			requests.filter((request) => request === 'GET /api/v1/settings/wallpaper').length,
			1
		);
		await authStore.logout();
		assert.equal(settingsStore.current.backgroundImage, null);
		await login('alice');
		assert.equal(
			settingsStore.current.backgroundImage,
			reference,
			'only this account sees its cached wallpaper'
		);
		await syncShareWallpaperOnLogin();
		await publishShareWallpaper();
		assert.equal(requests.filter((request) => request.endsWith('/wallpaper/image')).length, 1);
		assert.equal(
			requests.some((request) => request.startsWith('PUT')),
			false,
			'restored bytes must not be re-uploaded'
		);
		settingsStore.set({ ...settingsStore.current, backgroundImageMode: 'tile' });
		await publishShareWallpaper();
		assert.ok(requests.includes('PATCH /api/v1/settings/wallpaper'));
		settingsStore.set({ ...settingsStore.current, backgroundImage: null });
		await publishEditedWallpaper();
		assert.ok(requests.includes('DELETE /api/v1/settings/wallpaper'));
	});
});

test('logout, account switching and session expiry isolate all browser preferences', async () => {
	await withSession(async ({ setWallpaper, login }) => {
		setWallpaper('alice');
		await login('alice');
		await syncShareWallpaperOnLogin();
		settingsStore.set({ ...settingsStore.current, accentColor: '#123456', compactMode: true });
		await authStore.logout();
		assert.equal(settingsStore.current.accentColor, null);
		await login('bob');
		assert.equal(settingsStore.current.backgroundImage, null);
		assert.equal(settingsStore.current.accentColor, null);
		assert.equal(settingsStore.current.compactMode, false);
		await syncShareWallpaperOnLogin();
		settingsStore.set({ ...settingsStore.current, accentColor: '#abcdef' });
		authStore.handleRefreshFailure(new ApiRequestError('Expired', 401, 'UNAUTHORIZED'), 'refresh');
		assert.equal(settingsStore.current.accentColor, null);
		await login('alice');
		assert.equal(settingsStore.current.accentColor, '#123456');
		assert.equal(settingsStore.current.compactMode, true);
		await syncShareWallpaperOnLogin();
	});
});

test('remote replacement and deletion override an older browser wallpaper', async () => {
	await withSession(async ({ setWallpaper, login, stored, requests }) => {
		setWallpaper('alice', 'old');
		await login('alice');
		await syncShareWallpaperOnLogin();
		await authStore.logout();
		const replacement = setWallpaper('alice', 'new', 'tile');
		await login('alice');
		await syncShareWallpaperOnLogin();
		assert.equal(settingsStore.current.backgroundImage, replacement);
		assert.equal(settingsStore.current.backgroundImageMode, 'tile');
		await authStore.logout();
		stored.delete('alice');
		await login('alice');
		await syncShareWallpaperOnLogin();
		assert.equal(settingsStore.current.backgroundImage, null);
		assert.equal(settingsStore.current.frostedGlass, false);
		assert.equal(
			requests.some((request) => request.startsWith('PUT')),
			false
		);
	});
});

test('a save during login restoration wins and is published after the login request', async () => {
	await withSession(async ({ login, state, requests }) => {
		await login('alice');
		const started = deferred();
		const release = deferred();
		state.beforeMetadata = async () => {
			started.resolve();
			await release.promise;
		};
		const pending = syncShareWallpaperOnLogin();
		try {
			await started.promise;
			settingsStore.set({
				...settingsStore.current,
				backgroundImage: 'data:image/png;base64,aGVsbG8='
			});
			const publish = publishEditedWallpaper();
			release.resolve();
			await pending;
			await publish;
			assert.equal(settingsStore.current.backgroundImage, 'data:image/png;base64,aGVsbG8=');
			assert.ok(requests.includes('PUT /api/v1/settings/wallpaper'));
		} finally {
			release.resolve();
			await pending;
		}
	});
});

test('logout cancels an in-flight image download and queued publication before the next account', async () => {
	await withSession(async ({ login, setWallpaper, state, requests }) => {
		setWallpaper('alice');
		await login('alice');
		const started = deferred();
		const release = deferred();
		state.beforeImage = async () => {
			started.resolve();
			await release.promise;
		};
		const pending = syncShareWallpaperOnLogin();
		try {
			await started.promise;
			const cancelled = assert.rejects(publishShareWallpaper(), { name: 'AbortError' });
			await authStore.logout();
			await login('bob');
			release.resolve();
			await pending;
			await cancelled;
			await syncShareWallpaperOnLogin();
			assert.equal(settingsStore.current.backgroundImage, null);
			assert.equal(
				requests.some((request) => request.startsWith('PUT')),
				false
			);
		} finally {
			release.resolve();
			await pending;
		}
	});
});

test('late preference initialization cannot apply drive names after an account switch', async () => {
	await withSession(async ({ login, state }) => {
		const started = deferred();
		const release = deferred();
		let aliceReads = 0;
		state.beforeDriveNames = async (username) => {
			if (username === 'alice') {
				if (++aliceReads === 2) started.resolve();
				await release.promise;
			}
		};
		await login('alice');
		const aliceInitialization = settingsStore.initialize();
		try {
			await started.promise;
			await authStore.logout();
			await login('bob');
			await settingsStore.initialize();
			release.resolve();
			await aliceInitialization;
			assert.equal(settingsStore.current.driveNameOverrides.media, 'bob');
		} finally {
			release.resolve();
			await aliceInitialization;
		}
	});
});

test('legacy preferences are adopted only by their recorded owner', async () => {
	await withSession(async ({ login, values }) => {
		values.set(
			STORAGE_KEYS.SETTINGS,
			JSON.stringify({ accentColor: '#123456', compactMode: true })
		);
		values.set('boxbox_wallpaper_owner', 'alice');
		await login('bob');
		assert.equal(settingsStore.current.accentColor, null);
		await authStore.logout();
		await login('alice');
		assert.equal(settingsStore.current.accentColor, '#123456');
		assert.equal(settingsStore.current.compactMode, true);
	});
});

test('invalid metadata and image errors do not clear an existing wallpaper', async () => {
	await withSession(async ({ login, setWallpaper, state }) => {
		await login('alice');
		settingsStore.set({
			...settingsStore.current,
			backgroundImage: 'data:image/png;base64,aGVsbG8='
		});
		state.invalidMetadata = true;
		await assert.rejects(getWallpaper(), /Invalid wallpaper metadata/);
		state.invalidMetadata = false;
		setWallpaper('alice');
		state.imageStatus = 500;
		await assert.rejects(downloadWallpaper(), ApiRequestError);
		await syncShareWallpaperOnLogin();
		assert.equal(settingsStore.current.backgroundImage, 'data:image/png;base64,aGVsbG8=');
	});
});

test('an open settings draft adopts restored wallpaper without losing unrelated edits', async () => {
	await withSession(async ({ login, setWallpaper, requests }) => {
		const reference = setWallpaper('alice');
		await login('alice');
		const initial = settingsStore.current;
		const draft = { ...initial, compactMode: true };
		await syncShareWallpaperOnLogin();
		const refreshed = refreshWallpaperDraft(draft, initial, settingsStore.current);
		assert.equal(refreshed.backgroundImage, reference);
		assert.equal(refreshed.compactMode, true);
		settingsStore.set(refreshed);
		await publishShareWallpaper();
		assert.equal(
			requests.some((request) => request.startsWith('DELETE')),
			false
		);
		const edited = refreshWallpaperDraft(
			{
				...initial,
				backgroundImage: 'data:image/png;base64,aGVsbG8=',
				backgroundImageMode: 'tile'
			},
			initial,
			settingsStore.current
		);
		assert.equal(edited.backgroundImage, 'data:image/png;base64,aGVsbG8=');
		assert.equal(edited.backgroundImageMode, 'tile');
	});
});

test('a newly chosen wallpaper keeps its default fit and blur after late restoration', async () => {
	await withSession(async ({ login, setWallpaper }) => {
		setWallpaper('alice');
		await login('alice');
		const initial = settingsStore.current;
		assert.equal(initial.backgroundImageMode, 'cover');
		assert.equal(initial.frostedGlass, false);
		const draft = { ...initial, backgroundImage: 'data:image/png;base64,aGVsbG8=' };
		await syncShareWallpaperOnLogin();
		assert.equal(settingsStore.current.backgroundImageMode, 'contain');
		assert.equal(settingsStore.current.frostedGlass, true);
		const refreshed = refreshWallpaperDraft(draft, initial, settingsStore.current);
		assert.equal(refreshed.backgroundImage, draft.backgroundImage);
		assert.equal(refreshed.backgroundImageMode, 'cover');
		assert.equal(refreshed.frostedGlass, false);
	});
});

test('Reset to unchanged defaults wins over a pending login restoration and deletes remotely', async () => {
	await withSession(async ({ login, setWallpaper, state, stored, requests }) => {
		setWallpaper('alice');
		await login('alice');
		const started = deferred();
		const release = deferred();
		state.beforeMetadata = async () => {
			started.resolve();
			await release.promise;
		};
		const pending = syncShareWallpaperOnLogin();
		try {
			await started.promise;
			settingsStore.reset();
			const publish = publishEditedWallpaper();
			release.resolve();
			await pending;
			await publish;
			assert.equal(settingsStore.current.backgroundImage, null);
			assert.equal(stored.has('alice'), false);
			assert.ok(requests.includes('DELETE /api/v1/settings/wallpaper'));
		} finally {
			release.resolve();
			await pending;
		}
	});
});

test('failed persistence still applies preferences and a restored wallpaper in memory', async () => {
	await withSession(async ({ login, setWallpaper }) => {
		const reference = setWallpaper('alice');
		await login('alice');
		const original = localStorage.setItem;
		localStorage.setItem = () => {
			throw new Error('Storage full');
		};
		try {
			settingsStore.set({ ...settingsStore.current, compactMode: true });
			assert.equal(settingsStore.current.compactMode, true);
			await syncShareWallpaperOnLogin();
			assert.equal(settingsStore.current.backgroundImage, reference);
			assert.equal(settingsStore.current.compactMode, true);
		} finally {
			localStorage.setItem = original;
		}
	});
});

test('an unrelated save after failed image restoration preserves the remote wallpaper', async () => {
	await withSession(async ({ login, setWallpaper, state, stored, requests }) => {
		setWallpaper('alice');
		await login('alice');
		state.imageStatus = 500;
		await syncShareWallpaperOnLogin();
		settingsStore.set({ ...settingsStore.current, compactMode: true });
		await publishShareWallpaper();
		assert.equal(stored.has('alice'), true);
		assert.equal(
			requests.some((request) => request.startsWith('DELETE')),
			false
		);
	});
});

test('an unrelated save cannot upload stale cached bytes after newer remote image download fails', async () => {
	await withSession(async ({ login, setWallpaper, state, stored, requests }) => {
		await login('alice');
		setWallpaper('alice', 'cached-X');
		await syncShareWallpaperOnLogin();
		await authStore.logout();
		setWallpaper('alice', 'newer-Y');
		const newer = stored.get('alice');
		await login('alice');
		state.imageStatus = 500;
		await syncShareWallpaperOnLogin();
		const cached = settingsStore.current.backgroundImage;
		assert.ok(cached);
		assert.equal(await (await getLocalWallpaperBlob(cached))?.text(), 'cached-X');
		settingsStore.set({ ...settingsStore.current, compactMode: true });
		await publishShareWallpaper();
		assert.deepEqual(stored.get('alice'), newer);
		assert.equal(
			requests.some((request) => /^(PUT|DELETE) /.test(request)),
			false
		);
	});
});

test('a failed local upload survives login with no remote image and retries without crossing accounts', async () => {
	await withSession(async ({ login, state, stored, requests }) => {
		await login('pending-owner');
		await syncShareWallpaperOnLogin();
		const reference = await saveLocalWallpaperBlob(
			new Blob(['pending-local'], { type: 'image/png' })
		);
		settingsStore.set({
			...settingsStore.current,
			backgroundImage: reference,
			backgroundImageMode: 'tile'
		});
		state.uploadStatus = 500;
		await assert.rejects(publishEditedWallpaper(), ApiRequestError);
		await authStore.logout();
		await login('other-owner');
		await syncShareWallpaperOnLogin();
		assert.equal(settingsStore.current.backgroundImage, null);
		assert.equal(stored.has('other-owner'), false);
		await authStore.logout();
		await login('pending-owner');
		await syncShareWallpaperOnLogin();
		assert.equal(settingsStore.current.backgroundImage, reference);
		assert.equal(await (await getLocalWallpaperBlob(reference))?.text(), 'pending-local');
		assert.equal(stored.has('pending-owner'), false);
		state.uploadStatus = 200;
		resetShareWallpaperLoginSync();
		await syncShareWallpaperOnLogin();
		assert.equal(stored.get('pending-owner')?.source, reference);
		assert.equal(stored.get('pending-owner')?.mode, 'tile');
		assert.ok(requests.includes('PUT /api/v1/settings/wallpaper'));
		await authStore.logout();
		stored.delete('pending-owner');
		await login('pending-owner');
		await syncShareWallpaperOnLogin();
		assert.equal(
			settingsStore.current.backgroundImage,
			null,
			'a confirmed remote deletion wins after publication succeeds'
		);
		assert.equal(await getLocalWallpaperBlob(reference), null);
	});
});

test('successful wallpaper upload and deletion survive blocked pending-key cleanup', async () => {
	await withSession(async ({ login, stored, requests }) => {
		await login('cleanup-owner');
		await syncShareWallpaperOnLogin();
		const reference = await saveLocalWallpaperBlob(
			new Blob(['cleanup-image'], { type: 'image/png' })
		);
		settingsStore.set({ ...settingsStore.current, backgroundImage: reference });
		const original = localStorage.removeItem;
		localStorage.removeItem = () => {
			throw new DOMException('Storage blocked', 'SecurityError');
		};
		try {
			await publishEditedWallpaper();
			assert.equal(stored.get('cleanup-owner')?.source, reference);
			settingsStore.set({ ...settingsStore.current, backgroundImage: null });
			await publishEditedWallpaper();
			assert.equal(stored.has('cleanup-owner'), false);
			assert.ok(requests.includes('PUT /api/v1/settings/wallpaper'));
			assert.ok(requests.includes('DELETE /api/v1/settings/wallpaper'));
		} finally {
			localStorage.removeItem = original;
		}
	});
});

test('completed publication overrides a pending marker when cleanup keeps failing', async () => {
	await withSession(async ({ login, stored, requests }) => {
		await login('completed-owner');
		await syncShareWallpaperOnLogin();
		const reference = await saveLocalWallpaperBlob(new Blob(['completed'], { type: 'image/png' }));
		settingsStore.set({ ...settingsStore.current, backgroundImage: reference });
		const original = localStorage.removeItem;
		localStorage.removeItem = (key: string) => {
			if (key.startsWith('boxbox_wallpaper_pending:'))
				throw new DOMException('Blocked', 'SecurityError');
			original(key);
		};
		try {
			await publishEditedWallpaper();
			assert.equal(stored.get('completed-owner')?.source, reference);
			await authStore.logout();
			stored.delete('completed-owner');
			const uploads = requests.filter((request) => request.startsWith('PUT')).length;
			await login('completed-owner');
			await syncShareWallpaperOnLogin();
			assert.equal(settingsStore.current.backgroundImage, null);
			assert.equal(stored.has('completed-owner'), false);
			assert.equal(requests.filter((request) => request.startsWith('PUT')).length, uploads);
		} finally {
			localStorage.removeItem = original;
		}
	});
});

test('settings operations end on logout, same-user relogin and page replacement', async () => {
	await withSession(async ({ login }) => {
		await login('operation-owner');
		const { SettingsOperations } = await import('./settingsOperation');
		const page = new SettingsOperations();
		const save = page.start();
		assert.equal(save.aborted, false);
		const reset = page.start();
		assert.equal(save.aborted, true, 'Reset invalidates an earlier Save');
		await authStore.logout();
		assert.equal(reset.aborted, true);
		await login('operation-owner');
		assert.equal(page.start().aborted, true, 'same username does not revive an old page');
		const nextPage = new SettingsOperations();
		const picker = nextPage.start();
		nextPage.dispose();
		assert.equal(picker.aborted, true);
	});
});

test('existing persisted pending sources retry and become durable completed intent', async () => {
	await withSession(async ({ login, values, stored }) => {
		await login('persisted-owner');
		await syncShareWallpaperOnLogin();
		const reference = await saveLocalWallpaperBlob(new Blob(['persisted'], { type: 'image/png' }));
		settingsStore.set({ ...settingsStore.current, backgroundImage: reference });
		const pendingKey = 'boxbox_wallpaper_pending:persisted-owner';
		values.set(pendingKey, reference);
		await authStore.logout();
		await login('persisted-owner');
		await syncShareWallpaperOnLogin();
		assert.equal(stored.get('persisted-owner')?.source, reference);
		assert.equal(
			values.get(`${pendingKey}:${reference}`),
			'',
			'completion has a durable operation receipt'
		);
		assert.equal(wallpaperPublication('persisted-owner').kind, 'completed');
	});
});

test('applied image and removal intent survive cancellation while the publication queue is occupied', async () => {
	for (const remove of [false, true]) {
		await withSession(async ({ login, setWallpaper, state, stored }) => {
			const user = remove ? 'queued-removal' : 'queued-image';
			setWallpaper(user);
			await login(user);
			await syncShareWallpaperOnLogin();
			const gate = deferred();
			const started = deferred();
			let first = true;
			state.beforeMetadata = async () => {
				if (!first) return;
				first = false;
				started.resolve();
				await gate.promise;
			};
			const restoration = publishShareWallpaper();
			await started.promise;
			const source = remove ? null : await saveLocalWallpaperBlob(new Blob(['new-queued-image']));
			const intent = settingsStore.set({ ...settingsStore.current, backgroundImage: source }, true);
			const { SettingsOperations } = await import('./settingsOperation');
			const page = new SettingsOperations();
			const publication = assert.rejects(publishShareWallpaper(intent, false, page.start()), {
				name: 'AbortError'
			});
			page.dispose();
			gate.resolve();
			await restoration;
			await publication;
			assert.equal(wallpaperPublication(user).kind, 'pending');
			await authStore.logout();
			await login(user);
			await syncShareWallpaperOnLogin();
			assert.equal(settingsStore.current.backgroundImage, source);
			assert.equal(stored.get(user)?.source ?? null, source);
			assert.equal(wallpaperPublication(user).kind, 'completed');
		});
	}
});

test('failed browser writes retain owned completion but a new durable operation wins', async () => {
	await withSession(async ({ login, values, stored }) => {
		const user = 'fallback-owner';
		await login(user);
		await syncShareWallpaperOnLogin();
		const source = await saveLocalWallpaperBlob(new Blob(['fallback-image']));
		settingsStore.set({ ...settingsStore.current, backgroundImage: source }, true);
		const intent = wallpaperPublication(user);
		assert.equal(intent.kind, 'pending');
		const originalSet = localStorage.setItem;
		const originalGet = localStorage.getItem;
		localStorage.setItem = () => {
			throw new DOMException('Blocked', 'SecurityError');
		};
		try {
			await publishShareWallpaper(intent.kind === 'pending' ? intent.id : undefined);
			assert.equal(wallpaperPublication(user).kind, 'completed');
			localStorage.getItem = () => {
				throw new DOMException('Blocked', 'SecurityError');
			};
			assert.equal(wallpaperPublication(user).kind, 'completed');
			localStorage.getItem = originalGet;
			await authStore.logout();
			stored.delete(user);
			await login(user);
			await syncShareWallpaperOnLogin();
			assert.equal(settingsStore.current.backgroundImage, null);
			assert.equal(stored.has(user), false);
		} finally {
			localStorage.setItem = originalSet;
			localStorage.getItem = originalGet;
		}
		// Another page writes a distinct operation after the old local image was cleaned up.
		const nextSource = await saveLocalWallpaperBlob(new Blob(['new-fallback-image']));
		settingsStore.set({ ...settingsStore.current, backgroundImage: nextSource });
		values.set(
			`boxbox_wallpaper_pending:${user}`,
			JSON.stringify({ id: 'other-page', source: nextSource })
		);
		assert.deepEqual(wallpaperPublication(user), {
			kind: 'pending',
			id: 'other-page',
			source: nextSource
		});
		await publishShareWallpaper('other-page');
		assert.equal(stored.get(user)?.source, nextSource);
		assert.equal(wallpaperPublication(user).kind, 'completed');
	});
});

test('an external acknowledgement completes only its operation even with cached pending intent', async () => {
	await withSession(async ({ login, values }) => {
		const user = 'receipt-owner';
		await login(user);
		settingsStore.set(settingsStore.current, true);
		const intent = wallpaperPublication(user);
		assert.ok(intent.kind === 'pending');
		values.set(`boxbox_wallpaper_pending:${user}:${intent.id}`, '');
		assert.equal(wallpaperPublication(user).kind, 'completed');
		values.set(
			`boxbox_wallpaper_pending:${user}`,
			JSON.stringify({ id: 'new-operation', source: null })
		);
		assert.deepEqual(wallpaperPublication(user), {
			kind: 'pending',
			id: 'new-operation',
			source: null
		});
		const { completeWallpaperPublication } = await import('./wallpaperPublication');
		completeWallpaperPublication(user, intent.id);
		assert.deepEqual(wallpaperPublication(user), {
			kind: 'pending',
			id: 'new-operation',
			source: null
		});
	});
});

test('blocked intent and completion writes still apply and publish a new image in memory', async () => {
	await withSession(async ({ login, stored }) => {
		const user = 'all-writes-blocked';
		await login(user);
		await syncShareWallpaperOnLogin();
		const source = await saveLocalWallpaperBlob(new Blob(['memory-image']));
		const original = localStorage.setItem;
		localStorage.setItem = () => {
			throw new DOMException('Blocked', 'SecurityError');
		};
		try {
			const intent = settingsStore.set({ ...settingsStore.current, backgroundImage: source }, true);
			assert.equal(settingsStore.current.backgroundImage, source);
			assert.equal(wallpaperPublication(user).kind, 'pending');
			await publishShareWallpaper(intent);
			assert.equal(stored.get(user)?.source, source);
			assert.equal(wallpaperPublication(user).kind, 'completed');
		} finally {
			localStorage.setItem = original;
		}
	});
});

test('a queued operation cannot adopt a newer same-source intent', async () => {
	await withSession(async ({ login, setWallpaper, state, stored, requests }) => {
		const user = 'superseded-queue';
		setWallpaper(user);
		await login(user);
		await syncShareWallpaperOnLogin();
		const started = deferred();
		const gate = deferred();
		let first = true;
		state.beforeMetadata = async () => {
			if (!first) return;
			first = false;
			started.resolve();
			await gate.promise;
		};
		const occupying = publishShareWallpaper();
		await started.promise;
		const source = await saveLocalWallpaperBlob(new Blob(['superseded-image']));
		const oldIntent = settingsStore.set(
			{ ...settingsStore.current, backgroundImage: source },
			true
		);
		const queued = publishShareWallpaper(oldIntent);
		const nextIntent = settingsStore.set(
			{ ...settingsStore.current, backgroundImageMode: 'tile' },
			true
		);
		gate.resolve();
		await occupying;
		await queued;
		assert.deepEqual(wallpaperPublication(user), { kind: 'pending', id: nextIntent, source });
		assert.equal(requests.filter((request) => request.startsWith('PUT')).length, 0);
		await publishShareWallpaper(nextIntent);
		assert.equal(stored.get(user)?.source, source);
		assert.equal(stored.get(user)?.mode, 'tile');
		assert.equal(wallpaperPublication(user).kind, 'completed');
		// An older page can still hold different display preferences after B completes.
		settingsStore.set({ ...settingsStore.current, backgroundImageMode: 'contain' });
		const mutations = requests.filter((request) => /^(PUT|PATCH|DELETE) /.test(request)).length;
		await publishShareWallpaper(oldIntent, true);
		assert.equal(stored.get(user)?.mode, 'tile');
		assert.equal(
			requests.filter((request) => /^(PUT|PATCH|DELETE) /.test(request)).length,
			mutations
		);
	});
});
