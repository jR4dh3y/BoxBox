import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { results } from './common';
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
		await page.getByRole('button', { name: 'Sign In', exact: true }).click();
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

		await page.getByText('Parent.txt', { exact: true }).click();
		await page.keyboard.press('Home');
		await page.keyboard.press('Shift+End');
		assert.equal(await page.locator('[data-file-row="true"][aria-selected="true"]').count(), 2);
		await page.keyboard.press('Control+Home');
		assert.equal(await page.locator('[data-file-row="true"][aria-selected="true"]').count(), 2);
		await page.keyboard.press('Alt+Enter');
		await page.getByRole('complementary', { name: 'Selection details' }).waitFor();
		await page.getByRole('button', { name: 'Close details', exact: true }).click();
		await page.keyboard.press('F1');
		await page.getByRole('dialog', { name: 'Keyboard shortcuts' }).waitFor();
		await page.keyboard.press('Delete');
		assert.equal(await page.getByRole('dialog').count(), 1);
		await page.keyboard.press('Escape');
		await page.keyboard.press('Control+f');
		assert.equal(
			await page
				.getByRole('searchbox', { name: 'Search' })
				.evaluate((input) => input === document.activeElement),
			true
		);
		await page.getByTitle('Refresh', { exact: true }).focus();
		await page.keyboard.press('Escape');
		assert.equal(await page.locator('[data-file-row="true"][aria-selected="true"]').count(), 0);
		await page.keyboard.press('Control+a');
		assert.equal(await page.locator('[data-file-row="true"][aria-selected="true"]').count(), 2);
		await page.getByText('Child', { exact: true }).click();
		assert.equal(new URL(page.url()).searchParams.get('path'), 'perf/Navigation');
		await page.getByText('Child', { exact: true }).dblclick();
		await page.getByText('Nested.txt', { exact: true }).waitFor();
		await page.goBack();
		await page.getByText('Parent.txt', { exact: true }).waitFor();
		await page.goForward();
		await page.getByText('Nested.txt', { exact: true }).waitFor();
		await page.getByTitle('Back', { exact: true }).click();
		await page.getByText('Parent.txt', { exact: true }).waitFor();
		await page.getByTitle('Forward', { exact: true }).click();
		await page.getByText('Nested.txt', { exact: true }).waitFor();
		await page.getByTitle('Up', { exact: true }).click();
		await page.waitForURL((location) => location.searchParams.get('path') === 'perf/Navigation');
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
		await page.getByRole('button', { name: 'Choose Wallpaper', exact: true }).waitFor();
		await page.getByText('No shared links.', { exact: true }).waitFor();
		await page.reload();
		await page.getByRole('button', { name: 'Choose Wallpaper', exact: true }).waitFor();
		await page.locator('#file-open-behavior').selectOption('single-click');
		await page.getByRole('button', { name: 'Save', exact: true }).click();
		await page.getByRole('button', { name: 'Saved', exact: true }).waitFor();
		await page.goto(`${url}/settings`);
		assert.equal(await page.locator('#file-open-behavior').inputValue(), 'single-click');
		await page.goto(`${url}/browse?path=perf/Navigation`);
		await page.getByText('Child', { exact: true }).click();
		await page.getByText('Nested.txt', { exact: true }).waitFor();
		await page.goto(`${url}/settings`);

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

		await page.getByRole('button', { name: 'Logout', exact: true }).click();
		await page.waitForURL('**/login');
		await page.goto(`${url}/browse?path=perf/Navigation`);
		await page.waitForURL('**/login');
		assert.deepEqual(errors, [], 'no runtime errors during navigation or lazy component loading');
		console.log(
			'PASS: login, folder history, search focus, sort, grid, settings, public share, reload and logout'
		);
	} catch (error) {
		const page = context.pages()[0];
		if (page) {
			await page.screenshot({ path: join(results, 'navigation-failure.png') });
			writeFileSync(
				join(results, 'navigation-failure.json'),
				JSON.stringify(
					{
						url: page.url(),
						body: await page.locator('body').innerText(),
						active: await page.evaluate(() => document.activeElement?.outerHTML)
					},
					null,
					2
				)
			);
		}
		throw error;
	} finally {
		await context.close();
	}
}
