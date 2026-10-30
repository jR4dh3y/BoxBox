import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { Browser } from 'playwright';
import { startServer } from './server';

/** Exercise the loaded roots through the shared toolbar and its lazy editor. */
export async function verifyPathSuggestions(browser: Browser, binary: string) {
	const root = mkdtempSync(join(tmpdir(), 'boxbox-completion-'));
	const mount = join(root, 'mount');
	mkdirSync(join(mount, 'Completion'), { recursive: true });
	writeFileSync(join(mount, 'Completion', 'Complete.txt'), 'completion\n');
	const server = await startServer(binary, root, mount, 'completion');
	const { url, credentials } = server;
	const context = await browser.newContext();
	try {
		await context.request.post(`${url}/api/v1/auth/login`, { data: credentials });
		const page = await context.newPage();
		const errors: string[] = [];
		page.on('pageerror', (error) => errors.push(error.message));
		await page.goto(`${url}/browse?path=perf/Completion`);
		await page.getByText('Complete.txt', { exact: true }).waitFor();
		await page.keyboard.press('Control+l');
		const input = page.getByRole('combobox', { name: 'Location', exact: true });
		await input.fill('');
		await page.getByRole('option', { name: 'perf', exact: true }).waitFor();
		await input.fill('PE');
		await page.getByRole('option', { name: 'perf', exact: true }).waitFor();
		await input.press('Tab');
		assert.equal(await input.inputValue(), 'perf/');
		await page.getByRole('option', { name: 'perf/Completion', exact: true }).waitFor();
		await input.fill('per');
		await page.getByRole('option', { name: 'perf', exact: true }).waitFor();
		await input.press('ArrowRight');
		assert.equal(await input.inputValue(), 'perf/');
		await page.getByRole('option', { name: 'perf/Completion', exact: true }).click();
		await page.getByRole('button', { name: 'Edit location', exact: true }).waitFor();
		await page.getByText('Complete.txt', { exact: true }).waitFor();
		await page.getByRole('button', { name: 'Edit location', exact: true }).click();
		await input.fill('per');
		await page.getByRole('option', { name: 'perf', exact: true }).waitFor();
		await input.press('Enter');
		await page.waitForURL((value) => value.searchParams.get('path') === 'perf');
		assert.deepEqual(errors, [], 'root completion and child navigation have no runtime errors');
	} finally {
		await context.close();
		await server.stop();
		rmSync(root, { recursive: true, force: true });
	}
}
