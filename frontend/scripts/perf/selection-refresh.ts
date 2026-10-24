import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync, unlinkSync } from 'node:fs';
import { join } from 'node:path';
import type { Browser } from 'playwright';

export async function verifySelectionRefresh(
	browser: Browser,
	url: string,
	credentials: { username: string; password: string },
	mount: string
) {
	const folder = join(mount, 'Selection');
	mkdirSync(folder, { recursive: true });
	for (const name of ['kept.txt', 'gone.txt', '.hidden'])
		writeFileSync(join(folder, name), 'fixture\n');
	const context = await browser.newContext();
	try {
		await context.request.post(`${url}/api/v1/auth/login`, { data: credentials });
		const page = await context.newPage();
		const errors: string[] = [];
		page.on('pageerror', (error) => errors.push(error.message));
		await page.goto(`${url}/browse?path=perf/Selection`);
		await page.getByText('kept.txt', { exact: true }).click();
		await page.getByText('gone.txt', { exact: true }).click({ modifiers: ['Control'] });
		await page.getByText('2 of 2 selected', { exact: true }).waitFor();
		unlinkSync(join(folder, 'gone.txt'));
		await page.route('**/api/v1/files/list/perf/Selection', (route) =>
			route.fulfill({
				status: 503,
				contentType: 'application/json',
				body: JSON.stringify({ error: 'Selection fixture unavailable', code: 'INTERNAL_ERROR' })
			})
		);
		await page.getByTitle('Refresh', { exact: true }).click();
		await page.getByRole('alert').filter({ hasText: 'Selection fixture unavailable' }).waitFor();
		assert.equal(
			await page.getByText('2 of 2 selected', { exact: true }).count(),
			1,
			'failed refresh retains selection'
		);
		await page.unroute('**/api/v1/files/list/perf/Selection');
		await page.getByRole('button', { name: 'Retry', exact: true }).click();
		await page.getByText('1 of 1 selected', { exact: true }).waitFor();
		assert.equal(await page.getByText('gone.txt', { exact: true }).count(), 0);
		assert.equal(await page.locator('[data-file-row="true"][aria-selected="true"]').count(), 1);
		writeFileSync(join(folder, 'search-only.txt'), 'fixture\n');
		const search = page.getByRole('searchbox', { name: 'Search', exact: true });
		await search.fill('search-only');
		await page.getByText('search-only.txt', { exact: true }).click();
		await search.fill('');
		await page.getByText('kept.txt', { exact: true }).waitFor();
		assert.equal(
			await page.getByText('1 of 1 selected', { exact: true }).count(),
			1,
			'leaving search does not prune against an older directory response'
		);
		await page.getByTitle('Refresh', { exact: true }).click();
		await page.getByText('search-only.txt', { exact: true }).waitFor();
		assert.equal(await page.locator('[data-file-row="true"][aria-selected="true"]').count(), 1);
		mkdirSync(join(folder, 'Child'), { recursive: true });
		writeFileSync(join(folder, 'Child', 'nested-search.txt'), 'fixture\n');
		await search.fill('nested-search');
		await page.getByText('nested-search.txt', { exact: true }).click();
		await search.fill('');
		await page.getByText('kept.txt', { exact: true }).waitFor();
		const refreshed = page.waitForResponse(
			(response) => new URL(response.url()).pathname === '/api/v1/files/list/perf/Selection'
		);
		await page.getByTitle('Refresh', { exact: true }).click();
		await refreshed;
		await page.getByText('Child', { exact: true }).waitFor();
		await search.fill('nested-search');
		await page.getByText('nested-search.txt', { exact: true }).waitFor();
		assert.equal(
			await page.locator('[data-file-row="true"][aria-selected="true"]').count(),
			1,
			'refreshing the parent cannot prune a selected recursive search result'
		);
		assert.deepEqual(errors, []);
		console.log(
			'PASS: failed refresh preserves selection; confirmed refresh prunes deleted files and keeps existing selection'
		);
	} finally {
		await context.close();
	}
}
