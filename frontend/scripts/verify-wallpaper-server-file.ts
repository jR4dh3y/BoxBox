import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { chromium } from 'playwright';
import { startServer } from './perf/server';

// Run after scripts/build-single.sh; exercise the mounted-file picker and real storage.
const root = mkdtempSync(join(tmpdir(), 'boxbox-wallpaper-server-file-'));
const image = Buffer.from(
	'<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><path fill="red" d="M0 0h2v1H0z"/></svg>'
);
writeFileSync(join(root, 'wallpaper.svg'), image);
const server = await startServer(
	join(import.meta.dir, '../../dist/boxbox'),
	root,
	root,
	'wallpaper-server-file'
);
const browser = await chromium.launch({ headless: true });
try {
	const context = await browser.newContext();
	const login = await context.request.post(`${server.url}/api/v1/auth/login`, {
		data: server.credentials
	});
	assert.equal(login.status(), 200);
	const credentials: unknown = await login.json();
	assert.ok(
		typeof credentials === 'object' &&
			credentials !== null &&
			'accessToken' in credentials &&
			typeof credentials.accessToken === 'string'
	);
	const headers = { Authorization: `Bearer ${credentials.accessToken}` };
	const page = await context.newPage();
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	await page.goto(`${server.url}/settings`);
	await page.getByRole('button', { name: 'Choose Wallpaper', exact: true }).click();
	await page.getByRole('button', { name: /This Server/ }).click();
	await page.getByRole('button', { name: /perf Read only/ }).click();
	await page.getByRole('button', { name: /wallpaper.svg/ }).click();
	await page.locator('#wallpaper-preview-mode').selectOption('contain');
	await page.getByRole('button', { name: 'Use Wallpaper', exact: true }).click();
	const save = async () => {
		const start = performance.now();
		await page.getByRole('button', { name: 'Save', exact: true }).click();
		await page.getByRole('button', { name: 'Saved', exact: true }).waitFor();
		console.log(`Save click → rendered Saved: ${(performance.now() - start).toFixed(1)}ms`);
	};
	await save();
	const metadata = async () => {
		const response = await context.request.get(`${server.url}/api/v1/settings/wallpaper`, {
			headers
		});
		assert.equal(response.status(), 200);
		const value: unknown = await response.json();
		assert.ok(
			typeof value === 'object' &&
				value !== null &&
				'mode' in value &&
				'frostedGlass' in value &&
				'sha256' in value
		);
		return value;
	};
	const original = await metadata();
	assert.equal(original.mode, 'contain');
	writeFileSync(join(root, 'wallpaper.svg'), image.toString().replace('red', 'blue'));
	const fit = page
		.locator('div')
		.filter({ has: page.getByText('Wallpaper fit', { exact: true }) })
		.filter({ has: page.locator('select') })
		.last()
		.locator('select');
	// An unrelated save must leave the stored snapshot and display alone.
	await page.getByRole('switch', { name: 'Compact mode', exact: true }).click();
	await save();
	assert.deepEqual(await metadata(), original);
	await fit.selectOption('cover');
	await save();
	assert.equal((await metadata()).mode, 'cover', 'explicit default fit is preserved');
	await page.getByRole('switch', { name: 'Frosted glass blur', exact: true }).click();
	await save();
	const saved = await metadata();
	assert.equal(saved.mode, 'cover');
	assert.equal(saved.frostedGlass, true);
	assert.equal(
		saved.sha256,
		original.sha256,
		'display-only edits preserve the saved image snapshot'
	);
	const storedImage = await context.request.get(`${server.url}/api/v1/settings/wallpaper/image`, {
		headers
	});
	assert.deepEqual(await storedImage.body(), image);
	await page.reload();
	await page.locator('.app-shell[data-has-background="true"]').waitFor();
	assert.equal(await fit.inputValue(), 'cover');
	assert.equal(
		await page
			.getByRole('switch', { name: 'Frosted glass blur', exact: true })
			.getAttribute('aria-checked'),
		'true'
	);
	assert.deepEqual(errors, []);
	console.log(
		'PASS mounted server picker → changed bytes at same path → unrelated save protected → explicit fit/blur PATCH → reload restores display'
	);
	await context.close();
} finally {
	await browser.close();
	await server.stop();
	rmSync(root, { recursive: true, force: true });
}
