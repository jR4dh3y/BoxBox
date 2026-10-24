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
try {
	for (const scenario of ['draft', 'save-pending', 'reset', 'failed-image', 'storage-full']) {
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
			let release = () => {};
			let started = () => {};
			const gate = new Promise<void>((resolve) => {
				release = resolve;
			});
			const pending = new Promise<void>((resolve) => {
				started = resolve;
			});
			let first = true;
			await page.route('**/api/v1/settings/wallpaper', async (route) => {
				if (route.request().method() === 'GET' && first) {
					first = false;
					started();
					await gate;
				}
				await route.continue();
			});
			if (scenario === 'failed-image')
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
				if (scenario === 'reset') {
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
						scenario === 'failed-image'
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
