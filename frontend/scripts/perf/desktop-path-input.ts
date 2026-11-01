import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import type { Browser } from 'playwright';
import { startServer } from '../../../desktop/src/server';

export async function verifyDesktopPathInput(browser: Browser, binary: string) {
	const root = mkdtempSync('/tmp/boxbox-path-input-');
	writeFileSync(`${root}/typed-path.txt`, 'path fixture');
	const server = await startServer(binary, `${root}/state`);
	const context = await browser.newContext();
	try {
		const page = await context.newPage();
		page.setDefaultTimeout(15_000);
		await page.goto(server.url);
		await page.goto(
			`${new URL(page.url()).origin}/browse?${new URLSearchParams({ path: `root${root}` })}`
		);
		await page.getByText('typed-path.txt', { exact: true }).waitFor();
		const input = page.getByRole('combobox', { name: 'Location', exact: true });
		async function type(value: string) {
			await page.keyboard.press('Control+l');
			await input.fill('');
			for (let i = 0; i < value.length; i++) {
				await input.pressSequentially(value[i]);
				assert.equal(
					await input.inputValue(),
					value.slice(0, i + 1),
					'typing keeps native text and caret offsets'
				);
			}
		}
		await type('/tmp');
		await input.press('Enter');
		await page.waitForURL((url) => url.searchParams.get('path') === 'root/tmp');
		await type('~/Downloads');
		await input.press('Escape');
		await type('~/');
		await input.press('Enter');
		await page.waitForURL((url) => url.searchParams.get('path') === 'home');
		await type(root);
		await input.press('Enter');
		await page.getByText('typed-path.txt', { exact: true }).waitFor();
		await type(`file://${root}`);
		await input.press('Enter');
		await page.getByRole('button', { name: 'Edit location', exact: true }).waitFor();
		await page.getByText('typed-path.txt', { exact: true }).waitFor();
		console.log(
			'PASS: character-by-character absolute/home/file addresses preserve drafts and navigate real desktop mounts'
		);
	} finally {
		await context.close();
		await server.stop();
		rmSync(root, { recursive: true, force: true });
	}
}
