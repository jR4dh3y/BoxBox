import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import type { Browser } from 'playwright';

/** Exercises shared leases and invalidation against the production server, with held HTTP snapshots. */
export async function verifyBrowseOwnership(
	browser: Browser,
	url: string,
	credentials: { username: string; password: string },
	mount: string
) {
	const folder = join(mount, 'Ownership', 'Child');
	mkdirSync(folder, { recursive: true });
	writeFileSync(join(folder, 'seed.txt'), 'fixture');
	const context = await browser.newContext();
	const page = await context.newPage();
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	let release = () => {};
	try {
		await context.request.post(`${url}/api/v1/auth/login`, { data: credentials });
		let calls = 0;
		let captured = () => {};
		let started = new Promise<void>((resolve) => {
			captured = resolve;
		});
		let gate = new Promise<void>((resolve) => {
			release = resolve;
		});
		await page.route('**/api/v1/files/list/perf/Ownership/Child', async (route) => {
			calls++;
			const response = await route.fetch();
			if (calls === 1) {
				captured();
				await gate;
			}
			await route.fulfill({ response });
		});
		await page.goto(`${url}/browse?path=perf/Ownership/Child`, { waitUntil: 'commit' });
		await started;
		await page.getByRole('button', { name: 'Split view', exact: true }).click();
		assert.equal(
			await page.getByRole('region', { name: 'File browser pane', exact: true }).count(),
			2
		);
		await page.getByRole('button', { name: 'Split view', exact: true }).click();
		assert.equal(calls, 1, 'two panes share the pending listing');
		release();
		await page.getByText('seed.txt', { exact: true }).waitFor();
		assert.equal(calls, 1, 'closing one pane leaves the other request alive');

		calls = 0;
		started = new Promise<void>((resolve) => {
			captured = resolve;
		});
		gate = new Promise<void>((resolve) => {
			release = resolve;
		});
		await page.getByTitle('Refresh', { exact: true }).click();
		await started;
		writeFileSync(join(folder, 'queued.txt'), 'changed after captured response');
		await page.getByTitle('Refresh', { exact: true }).click();
		await page.getByTitle('Refresh', { exact: true }).click();
		release();
		await page.getByText('queued.txt', { exact: true }).waitFor();
		assert.equal(calls, 2, 'invalidation while pending queues one confirming request');
		await page.unroute('**/api/v1/files/list/perf/Ownership/Child');

		writeFileSync(join(folder, 'focus.txt'), 'focus confirmation');
		await page.evaluate(() => window.dispatchEvent(new Event('focus')));
		await page.getByText('focus.txt', { exact: true }).waitFor();
		await context.setOffline(true);
		writeFileSync(join(folder, 'reconnected.txt'), 'online confirmation');
		await context.setOffline(false);
		await page.getByText('reconnected.txt', { exact: true }).waitFor();

		started = new Promise<void>((resolve) => {
			captured = resolve;
		});
		gate = new Promise<void>((resolve) => {
			release = resolve;
		});
		await page.route('**/api/v1/files/list/perf/Ownership/Child', async (route) => {
			await route.fetch();
			captured();
			await gate;
		});
		const aborted = page.waitForEvent(
			'requestfailed',
			(request) => new URL(request.url()).pathname === '/api/v1/files/list/perf/Ownership/Child'
		);
		await page.getByTitle('Refresh', { exact: true }).click();
		await started;
		await page.getByTitle('Up', { exact: true }).click();
		await aborted;
		release();
		await page.getByText('Child', { exact: true }).waitFor();
		assert.deepEqual(errors, []);
		console.log(
			'PASS: split panes share pending listing; queued refresh confirms once; focus/reconnect update; final release aborts'
		);
	} finally {
		release();
		await context.close();
		rmSync(join(mount, 'Ownership'), { recursive: true, force: true });
	}
}
