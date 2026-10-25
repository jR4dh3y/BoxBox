import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import type { Browser } from 'playwright';
import { startServer } from '../../../desktop/src/server';

export async function verifyDesktopWallpaper(browser: Browser, binary: string) {
	const root = mkdtempSync('/tmp/boxbox-wallpaper-stage-');
	let server = await startServer(binary, `${root}/state`);
	const context = await browser.newContext();
	let release = () => {};
	try {
		const page = await context.newPage();
		page.setDefaultTimeout(15_000);
		await page.goto(server.url);
		const first = Buffer.from(
			'<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><path fill="red" d="M0 0h2v1H0z"/></svg>'
		);
		const second = Buffer.from(first.toString().replace('red', 'blue'));
		async function choose(image: Buffer) {
			await page.getByRole('button', { name: /^(Choose|Change) Wallpaper$/ }).click();
			await page
				.locator('input[type="file"]')
				.setInputFiles({ name: 'chosen.svg', mimeType: 'image/svg+xml', buffer: image });
			await page.getByRole('button', { name: 'Use Wallpaper', exact: true }).click();
		}
		async function savedReference() {
			const response = await context.request.get(
				`${new URL(page.url()).origin}/api/v1/desktop/preferences`
			);
			assert.equal(response.status(), 200);
			const value: unknown = await response.json();
			assert.ok(
				typeof value === 'object' &&
					value !== null &&
					'backgroundImage' in value &&
					typeof value.backgroundImage === 'string'
			);
			return value.backgroundImage;
		}
		await page.getByTitle('Settings', { exact: true }).click();
		await choose(first);
		const committed = page.waitForResponse(
			(r) => r.url().endsWith('/desktop/preferences') && r.request().method() === 'PUT'
		);
		await page.getByTitle('Save changes', { exact: true }).click();
		assert.equal((await committed).status(), 204);
		await page.getByRole('button', { name: 'Save', exact: true }).waitFor();
		const reference = await savedReference();
		assert.deepEqual(
			await (await context.request.get(`${new URL(page.url()).origin}${reference}`)).body(),
			first
		);
		let uploaded = () => {};
		const upload = new Promise<void>((resolve) => {
			uploaded = resolve;
		});
		const blocked = new Promise<void>((resolve) => {
			release = resolve;
		});
		let finished = () => {};
		const completion = new Promise<void>((resolve) => {
			finished = resolve;
		});
		await page.route('**/api/v1/desktop/wallpaper', async (route) => {
			if (route.request().method() !== 'PUT') return route.continue();
			try {
				const response = await route.fetch(); // The real Go server has already persisted the upload.
				assert.equal(response.status(), 200);
				uploaded();
				await blocked;
				await route.fulfill({ response }).catch(() => {}); // Page cancellation can close the request.
			} finally {
				finished();
			}
		});
		await choose(second);
		await page.getByTitle('Save changes', { exact: true }).click();
		await upload;
		await page.getByRole('button', { name: 'Back to files', exact: true }).click();
		await page.waitForURL('**/browse**');
		release();
		await completion;
		await page.waitForLoadState('networkidle');
		assert.equal(
			await savedReference(),
			reference,
			'abandoned Save cannot commit a staged reference'
		);
		assert.deepEqual(
			await (await context.request.get(`${new URL(page.url()).origin}${reference}`)).body(),
			first,
			'a completed late upload cannot replace bytes behind the committed URL'
		);
		await server.stop();
		server = await startServer(binary, `${root}/state`);
		await page.goto(server.url);
		await page.getByTitle('Settings', { exact: true }).click();
		assert.equal(await savedReference(), reference);
		assert.deepEqual(
			await (await context.request.get(`${new URL(page.url()).origin}${reference}`)).body(),
			first,
			'original bytes survive a backend restart and origin change'
		);
		await choose(second);
		const saved = page.waitForResponse(
			(r) => r.url().endsWith('/desktop/preferences') && r.request().method() === 'PUT'
		);
		await page.getByTitle('Save changes', { exact: true }).click();
		assert.equal((await saved).status(), 204);
		const next = await savedReference();
		assert.notEqual(next, reference);
		assert.deepEqual(
			await (await context.request.get(`${new URL(page.url()).origin}${next}`)).body(),
			second
		);
		assert.deepEqual(
			await (await context.request.get(`${new URL(page.url()).origin}${reference}`)).body(),
			first
		);
		console.log(
			'PASS: late desktop upload after Settings unmount preserves committed bytes across restart; explicit Save activates only its immutable image'
		);
	} finally {
		release();
		await context.close();
		await server.stop();
		rmSync(root, { recursive: true, force: true });
	}
}
