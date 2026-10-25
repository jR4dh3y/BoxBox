import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { chromium, type Page } from 'playwright';
import { startServer } from './perf/server';

const root = mkdtempSync(join(tmpdir(), 'boxbox-publication-'));
const server = await startServer(
	join(import.meta.dir, '../../dist/boxbox'),
	root,
	root,
	'publication'
);
const browser = await chromium.launch({ headless: true });
const image = Buffer.from(
	'<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><path fill="red" d="M0 0h2v1H0z"/></svg>'
);
const nextImage = Buffer.from(
	'<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><path fill="blue" d="M0 0h2v1H0z"/></svg>'
);
async function choose(page: Page, buffer = image) {
	await page.getByRole('button', { name: /^(Choose|Change) Wallpaper$/ }).click();
	await page
		.locator('input[type="file"]')
		.setInputFiles({ name: 'chosen.svg', mimeType: 'image/svg+xml', buffer });
	await page.getByRole('button', { name: 'Use Wallpaper', exact: true }).click();
}
async function logoutLogin(page: Page) {
	await page.getByRole('button', { name: 'Logout', exact: true }).click();
	await page.waitForURL('**/login**');
	await page.getByLabel('Username', { exact: true }).fill(server.credentials.username);
	await page.getByLabel('Password', { exact: true }).fill(server.credentials.password);
	const restoration = page.waitForResponse(
		(response) =>
			response.request().method() === 'GET' &&
			new URL(response.url()).pathname === '/api/v1/settings/wallpaper'
	);
	await page.getByRole('button', { name: 'Sign In', exact: true }).click();
	await restoration;
	await page.waitForURL('**/browse**');
	await page.waitForLoadState('networkidle');
}
try {
	for (const scenario of [
		'save-paint',
		'reset-queue',
		'clear-queue',
		'two-tabs',
		'overlap',
		'blocked-completion'
	]) {
		if (process.argv[2] && process.argv[2] !== scenario) continue;
		const context = await browser.newContext();
		if (scenario === 'save-paint')
			await context.addInitScript(() => {
				Reflect.deleteProperty(Crypto.prototype, 'randomUUID');
			});
		let release = () => {};
		try {
			const login = await context.request.post(`${server.url}/api/v1/auth/login`, {
				data: server.credentials
			});
			const token: unknown = await login.json();
			assert.ok(
				typeof token === 'object' &&
					token !== null &&
					'accessToken' in token &&
					typeof token.accessToken === 'string'
			);
			const headers = {
				Authorization: `Bearer ${token.accessToken}`,
				'Content-Type': 'image/svg+xml'
			};
			await context.request.delete(`${server.url}/api/v1/settings/wallpaper`, { headers });
			async function readRemote(path: string) {
				const login = await context.request.post(`${server.url}/api/v1/auth/login`, {
					data: server.credentials
				});
				const value: unknown = await login.json();
				assert.ok(
					typeof value === 'object' &&
						value !== null &&
						'accessToken' in value &&
						typeof value.accessToken === 'string'
				);
				return context.request.get(`${server.url}/api/v1/settings/${path}`, {
					headers: { Authorization: `Bearer ${value.accessToken}` }
				});
			}
			const page = await context.newPage();
			const errors: string[] = [];
			page.on('pageerror', (e) => errors.push(e.message));
			await page.goto(`${server.url}/settings`);
			await page.waitForLoadState('networkidle');
			await choose(page);
			if (scenario === 'save-paint') {
				// Hold the first paint after the preference is actually persisted, not before apply.
				await page.evaluate(() => {
					const frame = window.requestAnimationFrame.bind(window);
					window.requestAnimationFrame = (callback) => {
						const raw = localStorage.getItem('boxbox_settings:perf');
						const value: unknown = raw ? JSON.parse(raw) : null;
						if (
							typeof value === 'object' &&
							value !== null &&
							'backgroundImage' in value &&
							value.backgroundImage
						) {
							document.documentElement.dataset.paintHeld = 'true';
							window.addEventListener('release-paint', () => frame(callback), { once: true });
							return 0;
						}
						return frame(callback);
					};
				});
				await page.getByRole('button', { name: 'Save', exact: true }).click();
				await page.waitForFunction(() => document.documentElement.dataset.paintHeld === 'true');
				await page.getByRole('button', { name: 'Back to files', exact: true }).click();
				await page.waitForURL('**/browse**');
				await page.evaluate(() => window.dispatchEvent(new Event('release-paint')));
				const raw = await page.evaluate(() =>
					localStorage.getItem('boxbox_wallpaper_pending:perf')
				);
				assert.ok(raw, 'applied save must leave durable account intent before paint');
				await page.reload();
				await page.waitForLoadState('networkidle');
			} else {
				await page.getByRole('button', { name: 'Save', exact: true }).click();
				await page.getByRole('button', { name: 'Saved', exact: true }).waitFor();
				if (scenario === 'reset-queue' || scenario === 'clear-queue') {
					let started = () => {};
					const pending = new Promise<void>((done) => (started = done));
					const gate = new Promise<void>((done) => (release = done));
					let first = true;
					await page.route(
						(url) => url.pathname === '/api/v1/settings/wallpaper',
						async (route) => {
							if (first && route.request().method() === 'GET') {
								first = false;
								started();
								await gate;
							}
							await route.continue();
						}
					);
					await page.reload();
					await pending;
					if (scenario === 'reset-queue') {
						await page.getByRole('button', { name: 'Reset Defaults', exact: true }).click();
					} else {
						await page.getByRole('button', { name: 'Clear', exact: true }).click();
						await page.getByRole('button', { name: 'Save', exact: true }).click();
						await page.waitForFunction(() => {
							const raw = localStorage.getItem('boxbox_settings:perf');
							const value: unknown = raw ? JSON.parse(raw) : null;
							return (
								typeof value === 'object' &&
								value !== null &&
								'backgroundImage' in value &&
								value.backgroundImage === null
							);
						});
					}
					const intent = await page.evaluate(() => {
						const value: unknown = JSON.parse(
							localStorage.getItem('boxbox_wallpaper_pending:perf') ?? 'null'
						);
						return (
							typeof value === 'object' &&
							value !== null &&
							'source' in value &&
							value.source === null
						);
					});
					assert.equal(intent, true, 'applied removal must have durable intent before navigation');
					await page.getByRole('button', { name: 'Back to files', exact: true }).click();
					await page.waitForURL('**/browse**');
					release();
					await page.waitForLoadState('networkidle');
					await page.reload();
					await page.waitForLoadState('networkidle');
					assert.equal(
						(await readRemote('wallpaper')).status(),
						404,
						'abandoned queued reset must retry removal'
					);
					assert.equal(await page.locator('.app-shell[data-has-background="true"]').count(), 0);
				} else if (scenario === 'blocked-completion') {
					await choose(page, nextImage);
					await page.evaluate(() => {
						const set = Storage.prototype.setItem;
						Storage.prototype.setItem = function (key: string, value: string) {
							if (
								key.startsWith('boxbox_wallpaper_pending:') &&
								(value === '' || value.includes('completed'))
							)
								throw new DOMException('Blocked completion', 'SecurityError');
							set.call(this, key, value);
						};
						const remove = Storage.prototype.removeItem;
						Storage.prototype.removeItem = function (key: string) {
							if (key.startsWith('boxbox_wallpaper_pending:'))
								throw new DOMException('Blocked cleanup', 'SecurityError');
							remove.call(this, key);
						};
					});
					await page.getByRole('button', { name: 'Save', exact: true }).click();
					await page.getByRole('button', { name: 'Saved', exact: true }).waitFor();
					const deleted = await context.request.delete(`${server.url}/api/v1/settings/wallpaper`, {
						headers
					});
					assert.equal(deleted.status(), 200, 'independent client removes the completed image');
					await logoutLogin(page);
					await page.locator('.app-shell:not([data-has-background="true"])').waitFor();
					assert.equal(
						await page.locator('.app-shell[data-has-background="true"]').count(),
						0,
						'failed completion write fallback survives same-tab session change'
					);
					assert.equal((await readRemote('wallpaper')).status(), 404);
				} else {
					const other = await context.newPage();
					other.on('pageerror', (e) => errors.push(e.message));
					await other.goto(`${server.url}/settings`);
					await other.waitForLoadState('networkidle');
					await choose(other, nextImage);
					if (scenario === 'two-tabs') {
						await other.route(
							(url) => url.pathname === '/api/v1/settings/wallpaper',
							async (route) => {
								if (route.request().method() === 'PUT')
									await route.fulfill({
										status: 500,
										contentType: 'application/json',
										body: '{"error":"Unavailable","code":"INTERNAL_ERROR"}'
									});
								else await route.continue();
							}
						);
						await other.getByRole('button', { name: 'Save', exact: true }).click();
						await other.getByRole('button', { name: 'Failed', exact: true }).waitFor();
					} else {
						// A's old completion arrives after B has durably applied a newer edit.
						await choose(page, Buffer.from(image.toString().replace('red', 'green')));
						let started = () => {};
						const pending = new Promise<void>((done) => (started = done));
						const gate = new Promise<void>((done) => (release = done));
						await page.route(
							(url) => url.pathname === '/api/v1/settings/wallpaper',
							async (route) => {
								if (route.request().method() === 'PUT') {
									started();
									await gate;
								}
								await route.continue();
							}
						);
						await page.getByRole('button', { name: 'Save', exact: true }).click();
						await pending;
						await other.route(
							(url) => url.pathname === '/api/v1/settings/wallpaper',
							async (route) => {
								if (route.request().method() === 'PUT')
									await route.fulfill({
										status: 500,
										contentType: 'application/json',
										body: '{"error":"Unavailable","code":"INTERNAL_ERROR"}'
									});
								else await route.continue();
							}
						);
						await other.getByRole('button', { name: 'Save', exact: true }).click();
						await other.getByRole('button', { name: 'Failed', exact: true }).waitFor();
						release();
						await page.getByRole('button', { name: 'Saved', exact: true }).waitFor();
					}
					await logoutLogin(page);
					await page.locator('.app-shell[data-has-background="true"]').waitFor();
				}
			}
			if (scenario === 'save-paint' || scenario === 'two-tabs' || scenario === 'overlap') {
				const remote = await readRemote('wallpaper/image');
				assert.equal(remote.status(), 200);
				await page.locator('.app-shell[data-has-background="true"]').waitFor();
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
				assert.deepEqual(
					pixel,
					scenario === 'save-paint' ? [255, 0, 0, 255] : [0, 0, 255, 255],
					'rendered wallpaper must be the applied image'
				);
				assert.deepEqual(
					await remote.body(),
					scenario === 'save-paint' ? image : nextImage,
					'retry must publish the applied image'
				);
			}
			assert.deepEqual(errors, []);
			console.log(
				`PASS ${scenario}: mounted Settings → durable intent → session/reload → real API`
			);
		} finally {
			release();
			await context.close();
		}
	}
} finally {
	await browser.close();
	await server.stop();
	rmSync(root, { recursive: true, force: true });
}
