import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Browser } from 'playwright';
import { startServer } from '../../../desktop/src/server';

export async function verifyDesktopPreferences(browser: Browser, binary: string) {
	const root = mkdtempSync('/tmp/boxbox-desktop-preferences-');
	const data = join(root, 'state');
	let server = await startServer(binary, data);
	const context = await browser.newContext();
	let releasePending: (() => void) | undefined;
	try {
		const page = await context.newPage();
		page.setDefaultTimeout(15_000);
		writeFileSync(join(root, 'seed.txt'), 'fixture');
		let release: (() => void) | undefined;
		const pending = new Promise<void>((resolve) => {
			release = resolve;
			releasePending = resolve;
		});
		await page.route('**/api/v1/desktop/servers', (route) =>
			route.fulfill({
				json: {
					servers: [
						{
							id: '11111111-1111-4111-8111-111111111111',
							name: 'Offline',
							url: 'https://offline.example',
							expiresAt: new Date(Date.now() + 60_000).toISOString()
						}
					]
				}
			})
		);
		await page.route('**/api/v1/desktop/servers/*/api/v1/**', async (route) => {
			await pending;
			await route.fulfill({ status: 503, json: { error: 'Offline', code: 'INTERNAL_ERROR' } });
		});
		await page.goto(server.url);
		const firstOrigin = new URL(page.url()).origin;
		await page.goto(`${firstOrigin}/browse?${new URLSearchParams({ path: `root${root}` })}`);
		await page.getByText('seed.txt', { exact: true }).waitFor();
		await page.keyboard.press('Control+Shift+n');
		await page.getByRole('dialog').waitFor();
		await page.keyboard.press('Escape');
		release?.();
		await page.unroute('**/api/v1/desktop/servers/*/api/v1/**');
		await page.unroute('**/api/v1/desktop/servers');
		console.log('PASS: pending remote catalogs do not block local files or write actions');
		await page.getByTitle('Settings', { exact: true }).click();
		await page.getByRole('switch', { name: 'Compact mode', exact: true }).click();
		const saved = page.waitForResponse(
			(response) =>
				response.url().endsWith('/desktop/preferences') && response.request().method() === 'PUT'
		);
		await page.getByTitle('Save changes', { exact: true }).click();
		assert.equal((await saved).status(), 204);
		await server.stop();
		server = await startServer(binary, data);
		await page.goto(server.url);
		assert.notEqual(
			new URL(page.url()).origin,
			firstOrigin,
			'the restarted backend has a different origin'
		);
		await page.getByTitle('Settings', { exact: true }).click();
		await page.getByRole('switch', { name: 'Compact mode', exact: true }).waitFor();
		assert.equal(
			await page
				.getByRole('switch', { name: 'Compact mode', exact: true })
				.getAttribute('aria-checked'),
			'true'
		);
		console.log('PASS: desktop settings survive a backend restart and a new browser origin');
	} finally {
		releasePending?.();
		await context.close();
		await server.stop();
		rmSync(root, { recursive: true, force: true });
	}
}
