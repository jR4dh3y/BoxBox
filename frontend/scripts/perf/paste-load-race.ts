import assert from 'node:assert/strict';
import { accessSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Browser } from 'playwright';
import { chunkPath } from './lazy-load-errors';

/** A slow first import must not silently paste a later clipboard selection. */
export async function verifyPasteLoadRace(
	browser: Browser,
	url: string,
	credentials: { username: string; password: string },
	mount: string
) {
	const source = join(mount, 'PasteRace');
	mkdirSync(join(source, 'Dest'), { recursive: true });
	writeFileSync(join(source, 'First.txt'), 'first');
	writeFileSync(join(source, 'Second.txt'), 'second');
	const context = await browser.newContext();
	let release = () => {};
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	try {
		assert.equal(
			(await context.request.post(`${url}/api/v1/auth/login`, { data: credentials })).ok(),
			true
		);
		const page = await context.newPage();
		const errors: string[] = [];
		page.on('pageerror', (error) => errors.push(error.message));
		await page.goto(`${url}/browse?path=perf/PasteRace`);
		await page.getByText('First.txt', { exact: true }).waitFor();
		const moduleURL = `${url}/${chunkPath('src/lib/browse/pasteRuntime.svelte.ts')}`;
		await page.route(moduleURL, async (route) => {
			await gate;
			await route.continue();
		});
		const cut = async (name: string) => {
			const checkbox = page.getByRole('checkbox', { name: `Select ${name}`, exact: true });
			await checkbox.check();
			await checkbox.focus();
			await page.keyboard.press('Control+x');
		};
		const destination = async () => {
			await page.getByText('Dest', { exact: true }).dblclick();
			await page.waitForURL((next) => next.searchParams.get('path') === 'perf/PasteRace/Dest');
		};
		const paste = async () => {
			await page.getByRole('region', { name: 'File browser pane', exact: true }).focus();
			await page.keyboard.press('Control+v');
		};
		await cut('First.txt');
		await destination();
		const requested = page.waitForRequest(moduleURL, { timeout: 5000 });
		await paste();
		await requested;
		await page.getByTitle('Up', { exact: true }).click();
		await page.getByText('Second.txt', { exact: true }).waitFor();
		await cut('Second.txt');
		await destination();
		release();
		await page
			.getByText('The folder or clipboard changed. Paste again to use the current selection.', {
				exact: true
			})
			.waitFor();
		assert.equal(readFileSync(join(source, 'First.txt'), 'utf8'), 'first');
		assert.equal(readFileSync(join(source, 'Second.txt'), 'utf8'), 'second');
		assert.throws(() => accessSync(join(source, 'Dest', 'First.txt')));
		assert.throws(() => accessSync(join(source, 'Dest', 'Second.txt')));
		await paste();
		await page.getByText('Second.txt', { exact: true }).waitFor();
		assert.equal(readFileSync(join(source, 'Dest', 'Second.txt'), 'utf8'), 'second');
		assert.throws(() => accessSync(join(source, 'Second.txt')));
		assert.deepEqual(errors, []);
		console.log('PASS: delayed first paste preserves a changed clipboard until explicit retry');
	} finally {
		release();
		await context.close();
	}
}
