import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { chromium, type Page } from 'playwright';
import { startServer } from './perf/server';

const root = mkdtempSync(join(tmpdir(), 'boxbox-wallpaper-lifecycle-'));
const server = await startServer(
	join(import.meta.dir, '../../dist/boxbox'),
	root,
	root,
	'wallpaper-lifecycle',
	['other']
);
const browser = await chromium.launch({ headless: true });
const image = Buffer.from(
	'<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><path fill="red" d="M0 0h2v1H0z"/></svg>'
);

async function chooseImage(page: Page) {
	await page.getByRole('button', { name: 'Choose Wallpaper', exact: true }).click();
	await page
		.locator('input[type="file"]')
		.setInputFiles({ name: 'private.svg', mimeType: 'image/svg+xml', buffer: image });
	await page.getByRole('button', { name: 'Use Wallpaper', exact: true }).click();
}

async function signIn(page: Page, username: string) {
	await page.getByLabel('Username', { exact: true }).fill(username);
	await page.getByLabel('Password', { exact: true }).fill(server.credentials.password);
	await page.getByRole('button', { name: 'Sign In', exact: true }).click();
	await page.waitForURL('**/browse**');
}

try {
	for (const scenario of ['logout', 'switch', 'relogin', 'unmount', 'completed-reload']) {
		if (process.argv[2] && process.argv[2] !== scenario) continue;
		const context = await browser.newContext();
		try {
			const login = await context.request.post(`${server.url}/api/v1/auth/login`, {
				data: server.credentials
			});
			const credentials: unknown = await login.json();
			assert.ok(
				typeof credentials === 'object' &&
					credentials !== null &&
					'accessToken' in credentials &&
					typeof credentials.accessToken === 'string'
			);
			const headers = { Authorization: `Bearer ${credentials.accessToken}` };
			await context.request.delete(`${server.url}/api/v1/settings/wallpaper`, { headers });
			const page = await context.newPage();
			const errors: string[] = [];
			const mutations: string[] = [];
			page.on('pageerror', (error) => errors.push(error.message));
			page.on('request', (request) => {
				if (
					new URL(request.url()).pathname === '/api/v1/settings/wallpaper' &&
					request.method() !== 'GET'
				)
					mutations.push(request.method());
			});
			await page.goto(`${server.url}/settings`);
			await page.waitForLoadState('networkidle');
			await chooseImage(page);
			await page.getByRole('switch', { name: 'Compact mode', exact: true }).click();
			if (scenario === 'completed-reload') {
				await context.addInitScript(() => {
					const remove = Storage.prototype.removeItem;
					Storage.prototype.removeItem = function (key: string) {
						if (key.startsWith('boxbox_wallpaper_pending:'))
							throw new DOMException('Blocked cleanup', 'SecurityError');
						remove.call(this, key);
					};
				});
				await page.evaluate(() => {
					const remove = Storage.prototype.removeItem;
					Storage.prototype.removeItem = function (key: string) {
						if (key.startsWith('boxbox_wallpaper_pending:'))
							throw new DOMException('Blocked cleanup', 'SecurityError');
						remove.call(this, key);
					};
				});
				await page.getByRole('button', { name: 'Save', exact: true }).click();
				await page.getByRole('button', { name: 'Saved', exact: true }).waitFor();
				assert.deepEqual(mutations, ['PUT']);
				assert.equal(
					(
						await context.request.delete(`${server.url}/api/v1/settings/wallpaper`, { headers })
					).status(),
					200
				);
				await page.reload();
				await page.waitForLoadState('networkidle');
				assert.equal(await page.locator('.app-shell[data-has-background="true"]').count(), 0);
				assert.deepEqual(
					mutations,
					['PUT'],
					'completed publication cannot be retried after reload'
				);
			} else {
				await page.evaluate(() => {
					const descriptor = Object.getOwnPropertyDescriptor(
						IDBTransaction.prototype,
						'oncomplete'
					);
					if (!descriptor?.set) throw new Error('Missing IndexedDB completion setter');
					const setter = descriptor.set;
					let delayed = false;
					Object.defineProperty(IDBTransaction.prototype, 'oncomplete', {
						...descriptor,
						set(handler: (event: Event) => void) {
							if (this.mode === 'readwrite' && !delayed) {
								delayed = true;
								setter.call(this, (event: Event) => {
									document.documentElement.dataset.writeDelayed = 'true';
									window.addEventListener(
										'release-wallpaper-write',
										() => {
											handler.call(this, event);
											document.documentElement.dataset.writeReleased = 'true';
										},
										{ once: true }
									);
								});
							} else setter.call(this, handler);
						}
					});
				});
				await page.getByRole('button', { name: 'Save', exact: true }).click();
				await page.waitForFunction(() => document.documentElement.dataset.writeDelayed === 'true');
				if (scenario === 'unmount') {
					await page.getByRole('button', { name: 'Back to files', exact: true }).click();
					await page.waitForURL('**/browse**');
				} else {
					await page.getByRole('button', { name: 'Logout', exact: true }).click();
					await page.waitForURL('**/login**');
					if (scenario !== 'logout') await signIn(page, scenario === 'switch' ? 'other' : 'perf');
				}
				await page.evaluate(() => window.dispatchEvent(new Event('release-wallpaper-write')));
				await page.waitForFunction(() => document.documentElement.dataset.writeReleased === 'true');
				await page.waitForLoadState('networkidle');
				assert.equal(
					await page.locator('.app-shell[data-has-background="true"]').count(),
					0,
					'ended page/session cannot restore private image'
				);
				assert.deepEqual(mutations, [], 'ended operation cannot publish to either account');
				if (scenario !== 'logout') {
					await page.getByRole('button', { name: 'Settings', exact: true }).click();
					await page.waitForURL('**/settings');
					assert.equal(
						await page
							.getByRole('switch', { name: 'Compact mode', exact: true })
							.getAttribute('aria-checked'),
						'false'
					);
				}
				const preferences = await page.evaluate(() =>
					Object.entries(localStorage)
						.filter(([key]) => key.startsWith('boxbox_settings'))
						.map(([, value]) => {
							const parsed: unknown = JSON.parse(value);
							if (
								typeof parsed !== 'object' ||
								parsed === null ||
								!('backgroundImage' in parsed) ||
								!('compactMode' in parsed) ||
								(parsed.backgroundImage !== null && typeof parsed.backgroundImage !== 'string') ||
								typeof parsed.compactMode !== 'boolean'
							)
								throw new Error('Invalid account preferences');
							return { backgroundImage: parsed.backgroundImage, compactMode: parsed.compactMode };
						})
				);
				for (const value of preferences) {
					assert.equal(value.backgroundImage, null);
					assert.equal(value.compactMode, false);
				}
				const images = await page.evaluate(
					() =>
						new Promise<number>((resolve, reject) => {
							const request = indexedDB.open('boxbox-wallpapers');
							request.onsuccess = () => {
								const database = request.result;
								const count = database
									.transaction('wallpapers', 'readonly')
									.objectStore('wallpapers')
									.count();
								count.onsuccess = () => {
									database.close();
									resolve(count.result);
								};
								count.onerror = () => reject(count.error);
							};
							request.onerror = () => reject(request.error);
						})
				);
				assert.equal(images, 0, 'abandoned write cleans up only its staged image');
			}
			const readback = await context.request.post(`${server.url}/api/v1/auth/login`, {
				data: server.credentials
			});
			const readbackCredentials: unknown = await readback.json();
			assert.ok(
				typeof readbackCredentials === 'object' &&
					readbackCredentials !== null &&
					'accessToken' in readbackCredentials &&
					typeof readbackCredentials.accessToken === 'string'
			);
			assert.equal(
				(
					await context.request.get(`${server.url}/api/v1/settings/wallpaper`, {
						headers: { Authorization: `Bearer ${readbackCredentials.accessToken}` }
					})
				).status(),
				404
			);
			const other = await context.request.post(`${server.url}/api/v1/auth/login`, {
				data: { ...server.credentials, username: 'other' }
			});
			const otherCredentials: unknown = await other.json();
			assert.ok(
				typeof otherCredentials === 'object' &&
					otherCredentials !== null &&
					'accessToken' in otherCredentials &&
					typeof otherCredentials.accessToken === 'string'
			);
			assert.equal(
				(
					await context.request.get(`${server.url}/api/v1/settings/wallpaper`, {
						headers: { Authorization: `Bearer ${otherCredentials.accessToken}` }
					})
				).status(),
				404
			);
			assert.deepEqual(errors, []);
			console.log(
				`PASS ${scenario}: real Settings → delayed IndexedDB/session lifecycle → account preferences/API`
			);
		} finally {
			await context.close();
		}
	}
} finally {
	await browser.close();
	await server.stop();
	rmSync(root, { recursive: true, force: true });
}
