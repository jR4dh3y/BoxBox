import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync, rmSync, utimesSync } from 'node:fs';
import { join } from 'node:path';
import type { Browser } from 'playwright';

export async function verifyFolderNavigation(
	browser: Browser,
	url: string,
	credentials: { username: string; password: string },
	mount: string
) {
	// Keep navigable siblings outside the 10000-entry startup/virtualization fixture.
	const navigationRoot = join(mount, 'FolderNavigation');
	for (const folder of ['ScrollA', 'ScrollB']) {
		mkdirSync(join(navigationRoot, folder), { recursive: true });
		for (let i = 0; i < 500; i++) {
			const file = join(navigationRoot, folder, `File-${String(i).padStart(4, '0')}.txt`);
			writeFileSync(file, 'fixture');
			utimesSync(file, 1_700_000_000, 1_700_000_000);
		}
	}
	const context = await browser.newContext();
	try {
		await context.request.post(`${url}/api/v1/auth/login`, { data: credentials });
		const page = await context.newPage();
		page.setDefaultTimeout(15_000);
		await page.goto(`${url}/browse?path=perf/FolderNavigation/ScrollA`);
		const list = page.locator('.file-list-container');
		await page.getByText('File-0000.txt', { exact: true }).click();
		await page.keyboard.press('PageDown');
		const selectedIndex = await page
			.locator('[data-file-row][aria-selected="true"]')
			.getAttribute('data-file-index');
		assert.ok(Number(selectedIndex) > 1, 'PageDown moves file selection by a page');
		await page.keyboard.press('PageUp');
		assert.equal(
			await page.locator('[data-file-row][aria-selected="true"]').getAttribute('data-file-index'),
			'0'
		);
		await list.evaluate((node) => {
			node.scrollTop = 1800;
			node.dispatchEvent(new Event('scroll'));
		});
		await page.getByTitle('Up', { exact: true }).click();
		await page.getByText('ScrollB', { exact: true }).dblclick();
		await page.getByText('File-0000.txt', { exact: true }).waitFor();
		assert.equal(await list.evaluate((node) => node.scrollTop), 0, 'new folder starts at the top');
		await list.evaluate((node) => {
			node.scrollTop = 700;
			node.dispatchEvent(new Event('scroll'));
		});
		await page.getByTitle('Up', { exact: true }).click();
		await page.getByText('ScrollA', { exact: true }).dblclick();
		await page.waitForFunction(
			() => document.querySelector('.file-list-container')?.scrollTop === 1800
		);
		await page.getByRole('button', { name: 'Grid view', exact: true }).click();
		const grid = page.locator('[role="grid"]').locator('..');
		await page.locator('[data-file-item]').first().waitFor();
		await grid.evaluate((node) => {
			node.scrollTop = 1400;
			node.dispatchEvent(new Event('scroll'));
		});
		await page.getByRole('button', { name: 'List view', exact: true }).click();
		await page.waitForFunction(
			() => document.querySelector('.file-list-container')?.scrollTop === 1800
		);
		await page.getByRole('button', { name: 'Grid view', exact: true }).click();
		await page.waitForFunction(
			() => document.querySelector('[role="grid"]')?.parentElement?.scrollTop === 1400
		);
		console.log(
			'PASS: page keys move selection; folder and view scroll positions restore independently'
		);
	} finally {
		await context.close();
		rmSync(navigationRoot, { recursive: true, force: true });
	}
}
