import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { chromium, type Page } from 'playwright';
import { startServer } from './perf/server';

const root = mkdtempSync(join(tmpdir(), 'boxbox-picker-'));
mkdirSync(join(root, 'folder'));
const image = Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>');
writeFileSync(join(root, 'stale.svg'), image);
writeFileSync(join(root, 'folder', 'child.svg'), image);
const server = await startServer(join(import.meta.dir, '../../dist/boxbox'), root, root, 'picker', [
	'other'
]);
const browser = await chromium.launch({ headless: true });

function deferred() {
	let resolve = () => {};
	const promise = new Promise<void>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

// Fetch real API data, but control when each response reaches the mounted picker.
async function hold(page: Page, path: string) {
	const started = deferred();
	const release = deferred();
	const completed = deferred();
	await page.route(
		(url) => url.pathname === path,
		async (route) => {
			const response = await route.fetch();
			started.resolve();
			await release.promise;
			try {
				await route.fulfill({ response });
			} catch (error) {
				if (!(error instanceof Error) || !/closed|cancel|abort/i.test(error.message)) throw error;
			} finally {
				completed.resolve();
			}
		}
	);
	return { started: started.promise, release: release.resolve, completed: completed.promise };
}

async function settle(page: Page) {
	await page.evaluate(
		() =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
			)
	);
}

try {
	for (const scenario of [
		'overlap',
		'roots-overlap',
		'root-back',
		'local',
		'invalid-local',
		'switch',
		'unmount'
	]) {
		if (process.argv[2] && process.argv[2] !== scenario) continue;
		const context = await browser.newContext();
		const releases: (() => void)[] = [];
		try {
			await context.request.post(`${server.url}/api/v1/auth/login`, { data: server.credentials });
			const page = await context.newPage();
			const errors: string[] = [];
			page.on('pageerror', (error) => errors.push(error.message));
			await page.goto(`${server.url}/settings`);
			await page.waitForLoadState('networkidle');
			await page.getByRole('button', { name: 'Choose Wallpaper', exact: true }).click();
			const loading = page.getByText('Loading server folders...', { exact: true });
			if (scenario === 'roots-overlap') {
				const first = await hold(page, '/api/v1/files');
				releases.push(first.release);
				await page.getByRole('button', { name: /This Server/ }).click();
				await first.started;
				await page.getByRole('button', { name: 'Sources', exact: true }).click();
				const second = await hold(page, '/api/v1/files');
				releases.push(second.release);
				await page.getByRole('button', { name: /This Server/ }).click();
				await second.started;
				first.release();
				await first.completed;
				await settle(page);
				assert.equal(await loading.count(), 1, 'old roots load cannot clear the current load');
				second.release();
				await page.getByRole('button', { name: /perf Read only/ }).waitFor();
			} else {
				await page.getByRole('button', { name: /This Server/ }).click();
				await page.getByRole('button', { name: /perf Read only/ }).click();
				await page.getByRole('button', { name: /stale.svg/ }).waitFor();
				const first = await hold(page, '/api/v1/files/list/perf/folder');
				releases.push(first.release);
				const aborted = page.waitForEvent('requestfailed', {
					predicate: (request) =>
						new URL(request.url()).pathname === '/api/v1/files/list/perf/folder',
					timeout: 5000
				});
				// Observe after the UI assertions so the baseline fails on visible behavior first.
				const cancellation = aborted.then(
					() => true,
					() => false
				);
				await page.getByRole('button', { name: 'folder', exact: true }).click();
				await first.started;
				if (scenario === 'overlap') {
					const second = await hold(page, '/api/v1/files/list/perf');
					releases.push(second.release);
					await page.getByRole('button', { name: 'perf', exact: true }).click();
					await second.started;
					first.release();
					await first.completed;
					await settle(page);
					assert.equal(
						await loading.count(),
						1,
						'old folder cannot hide the pending breadcrumb load'
					);
					assert.equal(await page.getByRole('button', { name: /stale.svg/ }).count(), 0);
					second.release();
					await page.getByRole('button', { name: /stale.svg/ }).waitFor();
					assert.equal(await page.getByRole('button', { name: /child.svg/ }).count(), 0);
					assert.equal(await page.getByRole('button', { name: 'folder', exact: true }).count(), 1);
				} else if (scenario === 'root-back') {
					await page.getByRole('button', { name: 'Server', exact: true }).click();
					first.release();
					await first.completed;
					await page.getByRole('button', { name: /perf Read only/ }).waitFor();
					assert.equal(await page.getByRole('button', { name: /child.svg|stale.svg/ }).count(), 0);
				} else if (scenario === 'local' || scenario === 'invalid-local') {
					await page.getByRole('button', { name: 'Sources', exact: true }).click();
					await page.locator('input[type="file"]').setInputFiles({
						name: scenario === 'local' ? 'local.svg' : 'invalid.txt',
						mimeType: scenario === 'local' ? 'image/svg+xml' : 'text/plain',
						buffer: image
					});
					first.release();
					await first.completed;
					if (scenario === 'local') {
						await page.getByRole('button', { name: 'Use Wallpaper', exact: true }).waitFor();
						await page.getByRole('button', { name: 'Back', exact: true }).click();
						await page.getByRole('button', { name: /This Server/ }).click();
					} else {
						await page.getByText('Choose an image file.', { exact: true }).waitFor();
						await page.getByRole('button', { name: /This Server/ }).click();
					}
					await settle(page);
					assert.equal(await loading.count(), 0, 'local read cancellation must not strand loading');
					assert.equal(
						await page.getByRole('button', { name: /child.svg|stale.svg/ }).count(),
						0,
						'interrupted folder cannot expose old entries under the new path'
					);
					await page.getByRole('button', { name: 'perf', exact: true }).click();
					await page.getByRole('button', { name: /stale.svg/ }).waitFor();
				} else {
					await page.getByRole('button', { name: 'Close', exact: true }).click();
					if (scenario === 'unmount')
						await page.getByRole('button', { name: 'Back to files', exact: true }).click();
					else {
						await page.getByRole('button', { name: 'Logout', exact: true }).click();
						await page.waitForURL('**/login**');
						await page.getByLabel('Username', { exact: true }).fill('other');
						await page.getByLabel('Password', { exact: true }).fill(server.credentials.password);
						await page.getByRole('button', { name: 'Sign In', exact: true }).click();
						await page.waitForURL('**/browse**');
					}
					first.release();
					await first.completed;
					await page.getByRole('button', { name: 'Settings', exact: true }).click();
					await page.getByRole('button', { name: 'Choose Wallpaper', exact: true }).click();
					assert.equal(
						await page.getByRole('button', { name: 'Use Wallpaper', exact: true }).count(),
						0
					);
				}
				assert.equal(await cancellation, true, 'superseded directory request must be aborted');
			}
			assert.deepEqual(errors, []);
			console.log(`PASS picker ${scenario}: real API data, mounted UI ownership, no stale choice`);
		} finally {
			for (const release of releases) release();
			await context.close();
		}
	}
} finally {
	await browser.close();
	await server.stop();
	rmSync(root, { recursive: true, force: true });
}
