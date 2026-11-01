import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Browser } from 'playwright';

/** Exercise the APIs and language data excluded from the initial bundle. */
export async function verifyLazyFeatures(
	browser: Browser,
	url: string,
	credentials: { username: string; password: string },
	mount: string
) {
	const folder = join(mount, 'LazyFeatures');
	mkdirSync(folder);
	writeFileSync(join(folder, '00-preview.ts'), 'const answer: number = 42;\n');
	const context = await browser.newContext();
	try {
		const login = await context.request.post(`${url}/api/v1/auth/login`, { data: credentials });
		assert.equal(login.status(), 200);
		const page = await context.newPage();
		page.setDefaultTimeout(15_000);
		const errors: string[] = [];
		page.on('pageerror', (error) => errors.push(error.message));
		await page.goto(`${url}/browse?path=perf/LazyFeatures`);
		const file = page.getByText('00-preview.ts', { exact: true });
		await file.dblclick();
		await page.locator('.monaco-editor .view-lines').getByText('answer', { exact: true }).waitFor();
		await page.getByTitle('Close', { exact: true }).click();

		await file.click({ button: 'right' });
		await page.getByRole('menuitem', { name: 'Share file…', exact: true }).click();
		const share = page.getByRole('dialog').filter({
			has: page.getByRole('heading', { name: 'Share 00-preview.ts', exact: true })
		});
		await share.getByRole('button', { name: 'Create link', exact: true }).click();
		await share.getByRole('button', { name: 'Revoke link', exact: true }).waitFor();
		await share.getByRole('button', { name: 'Close', exact: true }).click();

		await page.getByTitle('Settings', { exact: true }).click();
		await page.getByRole('button', { name: 'Shared links', exact: true }).click();
		const row = page.getByRole('row').filter({ hasText: 'perf/LazyFeatures/00-preview.ts' });
		const revoked = page.waitForResponse(
			(response) => response.request().method() === 'DELETE' && response.url().includes('/shares/')
		);
		await row.getByRole('button', { name: 'Revoke link', exact: true }).click();
		const response = await revoked;
		assert.equal(response.status(), 200);
		assert.deepEqual(await response.json(), { success: true });
		await row.waitFor({ state: 'detached' });
		assert.deepEqual(errors, []);
		console.log('PASS: lazy code preview, share creation, settings listing and revocation');
	} finally {
		await context.close();
	}
}
