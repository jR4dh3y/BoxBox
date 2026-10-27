import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { chromium, type Page } from 'playwright';
import { startServer } from './perf/server';

const root = mkdtempSync(join(tmpdir(), 'boxbox-cache-ownership-'));
const other = 'other_%:分隔';
const image = Buffer.from(
	'<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><path fill="red" d="M0 0h2v1H0z"/></svg>'
);
async function cachedBytes(page: Page, reference: string) {
	return page.evaluate(async (reference) => {
		const database = await new Promise<IDBDatabase>((resolve, reject) => {
			const request = indexedDB.open('boxbox-wallpapers', 1);
			request.onsuccess = () => resolve(request.result);
			request.onerror = () => reject(request.error);
		});
		try {
			const value = await new Promise<unknown>((resolve, reject) => {
				const request = database
					.transaction('wallpapers')
					.objectStore('wallpapers')
					.get(reference.slice('boxbox-local-wallpaper:'.length));
				request.onsuccess = () => resolve(request.result);
				request.onerror = () => reject(request.error);
			});
			return typeof value === 'object' &&
				value !== null &&
				'blob' in value &&
				value.blob instanceof Blob
				? [...new Uint8Array(await value.blob.arrayBuffer())]
				: null;
		} finally {
			database.close();
		}
	}, reference);
}
async function preference(page: Page, primary: boolean) {
	return page.evaluate((primary) => {
		const key = Object.keys(localStorage).find(
			(key) =>
				key.startsWith('boxbox_settings:') &&
				(primary ? key === 'boxbox_settings:perf' : key !== 'boxbox_settings:perf')
		);
		const value: unknown = JSON.parse(key ? (localStorage.getItem(key) ?? 'null') : 'null');
		if (
			typeof value !== 'object' ||
			value === null ||
			!('backgroundImage' in value) ||
			typeof value.backgroundImage !== 'string'
		)
			throw new Error('Missing cached wallpaper preference');
		return value.backgroundImage;
	}, primary);
}
try {
	const server = await startServer(
		join(import.meta.dir, '../../dist/boxbox'),
		root,
		root,
		'cache-ownership',
		[other]
	);
	try {
		const browser = await chromium.launch({ headless: true });
		try {
			for (const scenario of ['clear', 'replace', 'reset', 'legacy', 'republish']) {
				if (process.argv[2] && process.argv[2] !== scenario) continue;
				const context = await browser.newContext();
				try {
					async function api(username: string) {
						const response = await context.request.post(`${server.url}/api/v1/auth/login`, {
							data: { username, password: server.credentials.password }
						});
						const value: unknown = await response.json();
						assert.ok(
							typeof value === 'object' &&
								value !== null &&
								'accessToken' in value &&
								typeof value.accessToken === 'string'
						);
						return {
							Authorization: `Bearer ${value.accessToken}`,
							'Content-Type': 'image/svg+xml'
						};
					}
					for (const user of ['perf', other]) {
						const response = await context.request.put(
							`${server.url}/api/v1/settings/wallpaper?mode=cover&frostedGlass=false`,
							{ headers: await api(user), data: image }
						);
						assert.equal(response.status(), 200);
					}
					const page = await context.newPage();
					const errors: string[] = [];
					page.on('pageerror', (error) => errors.push(error.message));
					await page.goto(`${server.url}/settings`);
					await page.locator('.app-shell[data-has-background="true"]').waitFor();
					await page.waitForLoadState('networkidle');
					async function switchTo(user: string) {
						await page.getByRole('button', { name: 'Logout', exact: true }).click();
						await page.waitForURL('**/login**');
						await page.getByLabel('Username', { exact: true }).fill(user);
						await page.getByLabel('Password', { exact: true }).fill(server.credentials.password);
						await page.getByRole('button', { name: 'Sign In', exact: true }).click();
						await page.waitForURL('**/browse**');
						await page.waitForLoadState('networkidle');
					}
					const bob = await preference(page, false);
					assert.deepEqual(await cachedBytes(page, bob), [...image]);
					await switchTo('perf');
					await page.goto(`${server.url}/settings`);
					await page.waitForLoadState('networkidle');
					const alice = await preference(page, true);
					async function reset() {
						const removed = page.waitForResponse(
							(response) =>
								response.request().method() === 'DELETE' &&
								new URL(response.url()).pathname === '/api/v1/settings/wallpaper'
						);
						await page.getByRole('button', { name: 'Reset Defaults', exact: true }).click();
						assert.equal((await removed).status(), 200);
						await page.locator('.app-shell:not([data-has-background="true"])').waitFor();
					}
					if (scenario === 'legacy') {
						// Seed the old version's shared hash record and both persisted references.
						const legacy = await page.evaluate(
							async ({ alice, bob }) => {
								const id = alice.slice('boxbox-local-wallpaper:'.length).split('.')[0];
								const reference = `boxbox-local-wallpaper:${id}`;
								const database = await new Promise<IDBDatabase>((resolve) => {
									const request = indexedDB.open('boxbox-wallpapers', 1);
									request.onsuccess = () => resolve(request.result);
								});
								try {
									await new Promise<void>((resolve, reject) => {
										const transaction = database.transaction('wallpapers', 'readwrite');
										const store = transaction.objectStore('wallpapers');
										const request = store.get(alice.slice('boxbox-local-wallpaper:'.length));
										request.onsuccess = () => {
											const value: unknown = request.result;
											if (typeof value !== 'object' || value === null || !('blob' in value))
												throw new Error('Missing migration bytes');
											store.put({ id, blob: value.blob });
											for (const ref of [alice, bob])
												if (ref !== reference)
													store.delete(ref.slice('boxbox-local-wallpaper:'.length));
										};
										transaction.oncomplete = () => resolve();
										transaction.onerror = () => reject(transaction.error);
									});
								} finally {
									database.close();
								}
								for (const key of Object.keys(localStorage).filter((key) =>
									key.startsWith('boxbox_settings:')
								)) {
									const value: unknown = JSON.parse(localStorage.getItem(key) ?? 'null');
									if (typeof value === 'object' && value !== null)
										localStorage.setItem(
											key,
											JSON.stringify({ ...value, backgroundImage: reference })
										);
								}
								return reference;
							},
							{ alice, bob }
						);
						await page.route('**/api/v1/settings/wallpaper/image', (route) => route.abort());
						await page.reload();
						await page.waitForLoadState('networkidle');
						await reset();
						assert.deepEqual(
							await cachedBytes(page, legacy),
							[...image],
							'legacy shared bytes survive account cleanup'
						);
					} else if (scenario === 'reset') {
						await reset();
					} else {
						if (scenario === 'clear' || scenario === 'republish')
							await page.getByRole('button', { name: 'Clear', exact: true }).click();
						else {
							await page.getByRole('button', { name: 'Change Wallpaper', exact: true }).click();
							await page.locator('input[type="file"]').setInputFiles({
								name: 'next.svg',
								mimeType: 'image/svg+xml',
								buffer: Buffer.from(image.toString().replace('red', 'blue'))
							});
							await page.getByRole('button', { name: 'Use Wallpaper', exact: true }).click();
						}
						await page.getByRole('button', { name: 'Save', exact: true }).click();
					}
					if (scenario !== 'reset' && scenario !== 'legacy')
						await page.getByRole('button', { name: 'Saved', exact: true }).waitFor();
					const bobRef = await preference(page, false);
					assert.deepEqual(
						await cachedBytes(page, bobRef),
						[...image],
						'B must retain bytes after A cleanup'
					);
					if (scenario !== 'legacy') {
						assert.notEqual(alice, bob, 'restored records belong to distinct accounts');
						assert.equal(await cachedBytes(page, alice), null, 'owned A cache is cleaned');
					}
					await page.route('**/api/v1/settings/wallpaper', async (route) => {
						if (route.request().method() === 'GET') await route.abort();
						else await route.continue();
					});
					await switchTo(other);
					await page.locator('.app-shell[data-has-background="true"]').waitFor();
					await page.goto(`${server.url}/settings`);
					await page.waitForLoadState('networkidle');
					await page.unroute('**/api/v1/settings/wallpaper');
					if (scenario === 'republish') {
						assert.equal(
							(
								await context.request.delete(`${server.url}/api/v1/settings/wallpaper`, {
									headers: await api(other)
								})
							).status(),
							200
						);
						await page.evaluate((source) => {
							const key = Object.keys(localStorage).find(
								(key) => key.startsWith('boxbox_settings:') && key !== 'boxbox_settings:perf'
							);
							if (!key) throw new Error('Missing account preference');
							localStorage.setItem(
								key.replace('boxbox_settings:', 'boxbox_wallpaper_pending:'),
								JSON.stringify({ id: 'cache-republish', source })
							);
						}, bobRef);
					}

					await page.getByRole('switch', { name: 'Frosted glass blur', exact: true }).click();
					await page.getByRole('button', { name: 'Save', exact: true }).click();
					await page.getByRole('button', { name: 'Saved', exact: true }).waitFor();
					const headers = await api(other);
					const remote = await context.request.get(
						`${server.url}/api/v1/settings/wallpaper/image`,
						{ headers }
					);
					assert.equal(remote.status(), 200);
					assert.deepEqual(await remote.body(), image);
					const metadata: unknown = await (
						await context.request.get(`${server.url}/api/v1/settings/wallpaper`, { headers })
					).json();
					assert.ok(
						typeof metadata === 'object' &&
							metadata !== null &&
							'frostedGlass' in metadata &&
							metadata.frostedGlass === true
					);
					assert.deepEqual(await cachedBytes(page, bobRef), [...image]);
					const pixel = await page.locator('.app-personal-background').evaluate(async (element) => {
						const image = new Image();
						image.src = getComputedStyle(element)
							.backgroundImage.slice(4, -1)
							.replace(/^["']|["']$/g, '');
						await image.decode();
						const canvas = document.createElement('canvas');
						canvas.width = 2;
						canvas.height = 1;
						const drawing = canvas.getContext('2d');
						if (!drawing) throw new Error('Canvas unavailable');
						drawing.drawImage(image, 0, 0);
						return [...drawing.getImageData(0, 0, 1, 1).data];
					});
					assert.deepEqual(pixel, [255, 0, 0, 255], 'B renders its original image after A cleanup');
					assert.deepEqual(errors, []);
					console.log(
						`PASS cache ${scenario}: same image, Unicode/separator account, real IndexedDB cleanup isolation, failed restore, display Save with exact bytes`
					);
				} finally {
					await context.close();
				}
			}
		} finally {
			await browser.close();
		}
	} finally {
		await server.stop();
	}
} finally {
	rmSync(root, { recursive: true, force: true });
}
