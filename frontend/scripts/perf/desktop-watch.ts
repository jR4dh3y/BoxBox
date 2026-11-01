import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, unlinkSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import type { Browser } from 'playwright';
import { startServer } from '../../../desktop/src/server';

export async function verifyDesktopWatch(browser: Browser, binary: string) {
	const folder = mkdtempSync(join(tmpdir(), 'boxbox-watch-browser-'));
	const server = await startServer(binary, join(folder, '.state'));
	const context = await browser.newContext({ viewport: { width: 1440, height: 1200 } });
	try {
		writeFileSync(join(folder, 'seed.txt'), 'seed\n');
		const page = await context.newPage();
		const errors: string[] = [];
		page.on('pageerror', (error) => errors.push(error.message));
		await page.goto(server.url);
		const origin = new URL(page.url()).origin;
		const path = `root${folder}`;
		const watched = page.waitForResponse(
			(response) => new URL(response.url()).pathname === '/api/v1/desktop/watch'
		);
		await page.goto(`${origin}/browse?${new URLSearchParams({ path })}`);
		await page.getByText('seed.txt', { exact: true }).waitFor();
		await watched;
		await page.getByText('seed.txt', { exact: true }).click();
		let active = 0,
			maxActive = 0,
			calls = 0;
		await page.route('**/api/v1/files/list/**', async (route) => {
			active++;
			calls++;
			maxActive = Math.max(maxActive, active);
			try {
				const response = await route.fetch();
				await new Promise((resolve) => setTimeout(resolve, 250));
				await route.fulfill({ response });
			} finally {
				active--;
			}
		});
		for (let index = 0; index < 20; index++) {
			writeFileSync(join(folder, `change-${String(index).padStart(2, '0')}.txt`), 'change\n');
			await new Promise((resolve) => setTimeout(resolve, 40));
		}
		unlinkSync(join(folder, 'seed.txt'));
		await page.getByText('change-19.txt', { exact: true }).waitFor();
		await page.getByRole('status').filter({ hasText: '20 items' }).waitFor();
		assert.equal(await page.getByText('seed.txt', { exact: true }).count(), 0);
		assert.equal(maxActive, 1, 'watcher refreshes never overlap or cancel each other');
		assert.ok(calls >= 2 && calls < 20, `coalesced ${calls} listing requests for 20 changes`);
		await page.unroute('**/api/v1/files/list/**');
		await page.getByRole('button', { name: 'Split view', exact: true }).click();
		assert.equal(
			await page.getByRole('region', { name: 'File browser pane', exact: true }).count(),
			2
		);
		await page.getByRole('button', { name: 'Split view', exact: true }).click();
		assert.equal(
			await page.getByRole('region', { name: 'File browser pane', exact: true }).count(),
			1
		);
		assert.deepEqual(errors, []);
		console.log(
			`PASS: native folder watcher refreshes 20 changes in ${calls} requests, prunes deleted selection, and supports split panes`
		);
	} finally {
		await context.close();
		await server.stop();
		rmSync(folder, { recursive: true, force: true });
	}
}
