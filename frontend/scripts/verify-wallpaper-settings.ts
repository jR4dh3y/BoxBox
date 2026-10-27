import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { chromium } from 'playwright';
import { startServer } from './perf/server';

// Run after scripts/build-single.sh. Each case uses a fresh browser and the real API.
const root = mkdtempSync(join(tmpdir(), 'boxbox-wallpaper-settings-'));
const server = await startServer(
	join(import.meta.dir, '../../dist/boxbox'),
	root,
	root,
	'wallpaper-settings'
);
const browser = await chromium.launch({ headless: true });
const image = Buffer.from(
	'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jB1sAAAAASUVORK5CYII=',
	'base64'
);
const chosenImage = Buffer.from(
	'<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><path fill="red" d="M0 0h2v1H0z"/></svg>'
);
try {
	for (const scenario of [
		'draft',
		'save-pending',
		'reset',
		'failed-image',
		'storage-full',
		'new-image',
		'stale-cache',
		'pending-upload',
		'blocked-cleanup'
	]) {
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
			const headers = {
				Authorization: `Bearer ${credentials.accessToken}`,
				'Content-Type': 'image/png'
			};
			const uploaded = await context.request.put(
				`${server.url}/api/v1/settings/wallpaper?mode=contain&frostedGlass=true&source=fixture`,
				{ headers, data: image }
			);
			assert.equal(uploaded.status(), 200);
			if (scenario === 'pending-upload') {
				assert.equal(
					(
						await context.request.delete(`${server.url}/api/v1/settings/wallpaper`, { headers })
					).status(),
					200
				);
			}
			if (scenario === 'storage-full')
				await context.addInitScript(() => {
					Storage.prototype.setItem = () => {
						throw new DOMException('Storage full', 'QuotaExceededError');
					};
				});
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
			if (scenario === 'stale-cache') {
				await page.goto(`${server.url}/settings`);
				await page.locator('.app-shell[data-has-background="true"]').waitFor();
				const replaced = await context.request.put(
					`${server.url}/api/v1/settings/wallpaper?mode=contain&frostedGlass=true&source=newer`,
					{ headers: { ...headers, 'Content-Type': 'image/svg+xml' }, data: chosenImage }
				);
				assert.equal(replaced.status(), 200);
			}
			let failUpload = scenario === 'pending-upload';
			let release = () => {};
			let started = () => {};
			const gate = new Promise<void>((resolve) => {
				release = resolve;
			});
			const pending = new Promise<void>((resolve) => {
				started = resolve;
			});
			let first = true;
			await page.route(
				(url) => url.pathname === '/api/v1/settings/wallpaper',
				async (route) => {
					if (route.request().method() === 'GET' && first) {
						first = false;
						started();
						await gate;
					}
					if (route.request().method() === 'PUT' && failUpload) {
						await route.fulfill({
							status: 500,
							contentType: 'application/json',
							body: '{"error":"Unavailable","code":"INTERNAL_ERROR"}'
						});
						return;
					}
					await route.continue();
				}
			);
			if (scenario === 'failed-image' || scenario === 'stale-cache')
				await page.route('**/api/v1/settings/wallpaper/image', (route) =>
					route.fulfill({
						status: 500,
						contentType: 'application/json',
						body: '{"error":"Unavailable","code":"INTERNAL_ERROR"}'
					})
				);
			await page.goto(`${server.url}/settings`);
			await pending;
			try {
				if (
					scenario === 'new-image' ||
					scenario === 'pending-upload' ||
					scenario === 'blocked-cleanup'
				) {
					await page.getByRole('button', { name: 'Choose Wallpaper', exact: true }).click();
					await page.locator('input[type="file"]').setInputFiles({
						name: 'chosen.svg',
						mimeType: 'image/svg+xml',
						buffer: chosenImage
					});
					await page.locator('#wallpaper-preview-mode').selectOption('cover');
					await page.getByRole('button', { name: 'Use Wallpaper', exact: true }).click();
					const loaded =
						scenario === 'pending-upload'
							? page.waitForResponse('**/api/v1/settings/wallpaper')
							: page.locator('.app-shell[data-has-background="true"]').waitFor();
					release();
					await loaded;
					const fit = page
						.locator('div')
						.filter({ has: page.getByText('Wallpaper fit', { exact: true }) })
						.filter({ has: page.locator('select') })
						.last()
						.locator('select');
					assert.equal(await fit.inputValue(), 'cover');
					assert.equal(
						await page
							.getByRole('switch', { name: 'Frosted glass blur', exact: true })
							.getAttribute('aria-checked'),
						'false'
					);
					if (scenario === 'blocked-cleanup')
						await page.evaluate(() => {
							Storage.prototype.removeItem = () => {
								throw new DOMException('Storage blocked', 'SecurityError');
							};
						});
					await page.getByRole('button', { name: 'Save', exact: true }).click();
					if (scenario === 'pending-upload') {
						await page.getByRole('button', { name: 'Failed', exact: true }).waitFor();
						const failedRetry = page.waitForResponse(
							(response) => response.request().method() === 'PUT' && response.status() === 500
						);
						await page.reload();
						await failedRetry;
						await page.locator('.app-shell[data-has-background="true"]').waitFor();
						assert.equal(
							(
								await context.request.get(`${server.url}/api/v1/settings/wallpaper`, { headers })
							).status(),
							404
						);
						failUpload = false;
						const retried = page.waitForResponse(
							(response) => response.request().method() === 'PUT' && response.status() === 200
						);
						await page.reload();
						await retried;
						await page.locator('.app-shell[data-has-background="true"]').waitFor();
						assert.deepEqual(mutations, ['PUT', 'PUT', 'PUT']);
					} else {
						await page.getByRole('button', { name: 'Saved', exact: true }).waitFor();
						assert.deepEqual(mutations, ['PUT']);
					}
					const savedImage = await context.request.get(
						`${server.url}/api/v1/settings/wallpaper/image`,
						{ headers }
					);
					assert.equal(savedImage.status(), 200);
					assert.deepEqual(await savedImage.body(), chosenImage);
				} else if (scenario === 'reset') {
					await page.getByRole('button', { name: 'Reset Defaults', exact: true }).click();
					const deleted = page.waitForResponse(
						(response) =>
							response.request().method() === 'DELETE' &&
							response.url().endsWith('/settings/wallpaper')
					);
					release();
					assert.equal((await deleted).status(), 200);
					assert.equal(await page.locator('.app-shell[data-has-background="true"]').count(), 0);
				} else {
					await page.getByRole('switch', { name: 'Compact mode', exact: true }).click();
					if (scenario === 'save-pending')
						await page.getByRole('button', { name: 'Save', exact: true }).click();
					const loaded =
						scenario === 'failed-image' || scenario === 'stale-cache'
							? page.waitForResponse('**/api/v1/settings/wallpaper/image')
							: page.locator('.app-shell[data-has-background="true"]').waitFor();
					release();
					await loaded;
					if (scenario !== 'save-pending')
						await page.getByRole('button', { name: 'Save', exact: true }).click();
					await page.getByRole('button', { name: 'Saved', exact: true }).waitFor();
					assert.equal(
						await page
							.getByRole('switch', { name: 'Compact mode', exact: true })
							.getAttribute('aria-checked'),
						'true'
					);
					assert.deepEqual(mutations, []);
				}
				const stored = await context.request.get(`${server.url}/api/v1/settings/wallpaper`, {
					headers
				});
				assert.equal(stored.status(), scenario === 'reset' ? 404 : 200);
				if (
					scenario === 'new-image' ||
					scenario === 'pending-upload' ||
					scenario === 'blocked-cleanup'
				) {
					const metadata: unknown = await stored.json();
					assert.ok(
						typeof metadata === 'object' &&
							metadata !== null &&
							'mode' in metadata &&
							'frostedGlass' in metadata
					);
					assert.equal(metadata.mode, 'cover');
					assert.equal(metadata.frostedGlass, false);
				}
				if (scenario === 'stale-cache') {
					// Failed replacement keeps the last usable image visible until a retry succeeds.
					await page.locator('.app-shell[data-has-background="true"]').waitFor();
					assert.match(
						await page
							.locator('.app-personal-background')
							.evaluate((element) => getComputedStyle(element).backgroundImage),
						/^url\("?blob:/
					);
					const remoteImage = await context.request.get(
						`${server.url}/api/v1/settings/wallpaper/image`,
						{ headers }
					);
					assert.deepEqual(await remoteImage.body(), chosenImage);
					await page.unroute('**/api/v1/settings/wallpaper/image');
					const retried = page.waitForResponse('**/api/v1/settings/wallpaper/image');
					await page.reload();
					assert.equal((await retried).status(), 200);
					await page.locator('.app-shell[data-has-background="true"]').waitFor();
					assert.deepEqual(mutations, []);
				}
				assert.deepEqual(errors, []);
				console.log(`PASS ${scenario}: settings UI → real API → stored wallpaper`);
			} finally {
				release();
			}
		} finally {
			await context.close();
		}
	}
} finally {
	await browser.close();
	await server.stop();
	rmSync(root, { recursive: true, force: true });
}
