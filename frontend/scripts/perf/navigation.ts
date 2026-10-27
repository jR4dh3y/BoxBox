import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Browser } from 'playwright';

export async function verifyNavigation(
	browser: Browser,
	url: string,
	credentials: { username: string; password: string },
	mount: string
) {
	// Add navigation fixtures after timing samples so they cannot change the measured folder.
	mkdirSync(join(mount, 'Navigation', 'Child'), { recursive: true });
	writeFileSync(join(mount, 'Navigation', 'Parent.txt'), 'parent\n');
	writeFileSync(join(mount, 'Navigation', 'Child', 'Nested.txt'), 'nested\n');
	const context = await browser.newContext();
	const page = await context.newPage();
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	try {
		await page.goto(`${url}/browse?path=perf/Navigation`);
		await page.waitForURL('**/login');
		await page.getByLabel('Username', { exact: true }).fill(credentials.username);
		await page.getByLabel('Password', { exact: true }).fill(credentials.password);
		const loginResponse = page.waitForResponse(
			(response) => new URL(response.url()).pathname === '/api/v1/auth/login'
		);
		await page.getByRole('button', { name: 'Sign in', exact: true }).click();
		await page.waitForURL('**/browse');
		const session: unknown = await (await loginResponse).json();
		assert.ok(
			typeof session === 'object' &&
				session !== null &&
				'accessToken' in session &&
				typeof session.accessToken === 'string'
		);

		await page.goto(`${url}/browse?path=perf/Navigation`);
		await page.getByText('Parent.txt', { exact: true }).waitFor();
		const childRow = page
			.locator('[data-file-row="true"]')
			.filter({ has: page.getByText('Child', { exact: true }) });
		for (const action of ['Pin to Favorites', 'Unpin from Favorites']) {
			await childRow.click({ button: 'right' });
			await page.getByRole('menuitem', { name: action, exact: true }).click();
			await page
				.getByRole('alert')
				.filter({
					hasText:
						action === 'Pin to Favorites'
							? 'Child pinned to favorites'
							: 'Child unpinned from favorites'
				})
				.waitFor();
			await page.getByRole('button', { name: 'Dismiss notification', exact: true }).click();
		}

		await page.getByText('Child', { exact: true }).click();
		await page.getByText('Nested.txt', { exact: true }).waitFor();
		await page.getByTitle('Back', { exact: true }).click();
		await page.getByText('Parent.txt', { exact: true }).waitFor();
		await page.getByTitle('Forward', { exact: true }).click();
		await page.getByText('Nested.txt', { exact: true }).waitFor();
		await page.getByRole('button', { name: 'Navigation', exact: true }).click();
		await page.getByText('Parent.txt', { exact: true }).waitFor();

		const historyLength = await page.evaluate(() => history.length);
		const search = page.getByRole('searchbox', { name: 'Search', exact: true });
		await search.fill('Parent');
		await page.waitForURL((location) => location.searchParams.get('q') === 'Parent');
		assert.equal(await search.evaluate((input) => document.activeElement === input), true);
		assert.equal(await page.evaluate(() => history.length), historyLength);
		await page.getByRole('button', { name: 'Clear search', exact: true }).click();
		await page.waitForURL((location) => !location.searchParams.has('q'));
		await page.getByRole('columnheader', { name: 'Size', exact: false }).click();
		await page.waitForURL((location) => location.searchParams.get('sort') === 'size');
		assert.equal(await page.evaluate(() => history.length), historyLength);

		await page.getByRole('button', { name: 'Grid view', exact: true }).click();
		await page.locator('[data-file-item="true"]').first().waitFor();
		assert.equal(new URL(page.url()).searchParams.get('view'), 'grid');
		assert.equal(await page.evaluate(() => history.length), historyLength);
		await page.reload();
		await page.locator('[data-file-item="true"]').first().waitFor();
		await page.getByRole('button', { name: 'List view', exact: true }).click();
		await page.locator('[data-file-row="true"]').first().waitFor();

		await page.getByTitle('Settings', { exact: true }).click();
		const sections = page.getByRole('navigation', { name: 'Settings sections' });
		await sections.getByRole('button', { name: 'Appearance', exact: true }).click();
		await page.getByRole('button', { name: 'Choose', exact: true }).waitFor();
		await sections.getByRole('button', { name: 'Shared links', exact: true }).click();
		await page.getByText('No shared links.', { exact: true }).waitFor();
		await page.reload();
		await page.getByTitle('Settings', { exact: true }).click();
		await sections.getByRole('button', { name: 'Appearance', exact: true }).click();
		await page.getByRole('button', { name: 'Choose', exact: true }).waitFor();
		await page.getByRole('button', { name: 'Close', exact: true }).click();

		const created = await context.request.post(`${url}/api/v1/shares`, {
			headers: { Authorization: `Bearer ${session.accessToken}` },
			data: { path: 'perf/Navigation', permissions: { view: true, download: true } }
		});
		assert.equal(created.status(), 201);
		const share: unknown = await created.json();
		assert.ok(
			typeof share === 'object' && share !== null && 'url' in share && typeof share.url === 'string'
		);
		assert.match(share.url, /^\/s\/[\w-]+$/);
		const recipient = await browser.newContext();
		try {
			const sharedPage = await recipient.newPage();
			sharedPage.on('pageerror', (error) => errors.push(error.message));
			await sharedPage.goto(`${url}${share.url}`);
			await sharedPage.getByText('Parent.txt', { exact: true }).waitFor();
			await sharedPage.reload();
			await sharedPage.getByText('Parent.txt', { exact: true }).waitFor();
		} finally {
			await recipient.close();
		}

		await page.getByTitle('Settings', { exact: true }).click();
		await sections.getByRole('button', { name: 'Account', exact: true }).click();
		await page.getByRole('button', { name: 'Sign out', exact: true }).click();
		await page.waitForURL('**/login');
		await page.goto(`${url}/browse?path=perf/Navigation`);
		await page.waitForURL('**/login');
		assert.deepEqual(errors, [], 'no runtime errors during navigation or lazy component loading');
		console.log(
			'PASS: login, folder history, search focus, sort, grid, settings, public share, reload and logout'
		);
	} finally {
		await context.close();
	}
}
