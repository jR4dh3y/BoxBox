import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium, request, type Page } from 'playwright';
import { getLocalWallpaperId } from '../src/lib/utils/wallpaperStorage';

// Use an isolated server with alice and bob, both using wallpaper-test.
const baseURL = process.env.BOXBOX_TEST_URL ?? 'http://127.0.0.1:5182';
const output = resolve(process.env.BOXBOX_TEST_EVIDENCE ?? '../docs/evidence/issue-181');
mkdirSync(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
const first = await browser.newContext({ viewport: { width: 1280, height: 800 } });
const second = await browser.newContext({ viewport: { width: 1280, height: 800 } });
const failures: string[] = [];
const requests: { client: string; method: string; path: string; status: number }[] = [];
const checks: string[] = [];
function observe(page: Page, client: string) {
	page.on('pageerror', (error) => failures.push(error.message));
	page.on('response', (response) => {
		const path = new URL(response.url()).pathname;
		if (path.startsWith('/api/v1/settings/wallpaper'))
			requests.push({
				client,
				method: response.request().method(),
				path,
				status: response.status()
			});
	});
}
async function login(page: Page, username: string) {
	await page.getByLabel('Username', { exact: true }).fill(username);
	await page.getByLabel('Password', { exact: true }).fill('wallpaper-test');
	await page.getByRole('button', { name: 'Sign In', exact: true }).click();
	await page.waitForURL('**/browse');
}
async function wallpaperRendered(page: Page) {
	await page
		.locator(
			'.app-shell[data-has-background="true"][data-background-mode="contain"][data-frosted-glass="true"]'
		)
		.waitFor();
	await page.waitForFunction(async () => {
		const element = document.querySelector('.app-personal-background');
		if (!element) return false;
		const source = getComputedStyle(element).backgroundImage.match(/^url\("(.+)"\)$/)?.[1];
		if (!source) return false;
		const image = new Image();
		image.src = source;
		try {
			await image.decode();
			return true;
		} catch {
			return false;
		}
	});
}
async function logout(page: Page) {
	await page.goto(`${baseURL}/settings`);
	await page.getByRole('button', { name: 'Logout', exact: true }).click();
	await page.waitForURL('**/login');
	assert.equal(await page.locator('.app-personal-background').count(), 0);
}
try {
	const fixture = await request.newContext({ baseURL });
	try {
		for (const username of ['alice', 'bob']) {
			const response = await fixture.post('/api/v1/auth/login', {
				data: { username, password: 'wallpaper-test' }
			});
			assert.equal(response.status(), 200);
			const value: unknown = await response.json();
			assert.ok(
				typeof value === 'object' &&
					value !== null &&
					'accessToken' in value &&
					typeof value.accessToken === 'string'
			);
			const removed = await fixture.delete('/api/v1/settings/wallpaper', {
				headers: { Authorization: `Bearer ${value.accessToken}` }
			});
			assert.equal(removed.status(), 200);
		}
	} finally {
		await fixture.dispose();
	}
	const device = await first.newPage();
	observe(device, 'device-one');
	await device.goto(baseURL);
	await login(device, 'alice');
	await device.goto(`${baseURL}/settings`);
	await device.getByRole('button', { name: 'Personalization', exact: true }).click();
	await device.getByRole('button', { name: 'Choose Wallpaper', exact: true }).click();
	await device.getByRole('button', { name: /This Device/ }).waitFor();
	const dataURL = await device.evaluate(() => {
		const canvas = document.createElement('canvas');
		canvas.width = 1280;
		canvas.height = 800;
		const drawing = canvas.getContext('2d');
		if (!drawing) throw new Error('Canvas is unavailable');
		const gradient = drawing.createLinearGradient(0, 0, 1280, 800);
		gradient.addColorStop(0, '#163c65');
		gradient.addColorStop(0.5, '#623d6b');
		gradient.addColorStop(1, '#eaa269');
		drawing.fillStyle = gradient;
		drawing.fillRect(0, 0, 1280, 800);
		drawing.fillStyle = '#e8b6a0';
		drawing.beginPath();
		drawing.arc(960, 240, 110, 0, Math.PI * 2);
		drawing.fill();
		drawing.fillStyle = '#173950';
		drawing.beginPath();
		drawing.moveTo(0, 650);
		drawing.lineTo(350, 250);
		drawing.lineTo(720, 700);
		drawing.lineTo(1040, 440);
		drawing.lineTo(1280, 680);
		drawing.lineTo(1280, 800);
		drawing.lineTo(0, 800);
		drawing.fill();
		return canvas.toDataURL('image/png');
	});
	const image = Buffer.from(dataURL.split(',')[1] ?? '', 'base64');
	const digest = createHash('sha256').update(image).digest('hex');
	await device
		.locator('input[type="file"]')
		.setInputFiles({ name: 'alice-wallpaper.png', mimeType: 'image/png', buffer: image });
	await device.getByRole('button', { name: 'Use Wallpaper', exact: true }).click();
	await device
		.locator('select')
		.filter({ has: device.locator('option[value="contain"]') })
		.selectOption('contain');
	await device.getByRole('switch', { name: 'Frosted glass blur', exact: true }).click();
	const upload = device.waitForResponse(
		(response) =>
			response.request().method() === 'PUT' &&
			new URL(response.url()).pathname === '/api/v1/settings/wallpaper'
	);
	await device.getByRole('button', { name: 'Save', exact: true }).click();
	assert.equal((await upload).status(), 200);
	await device.getByRole('button', { name: 'Saved', exact: true }).waitFor();
	await device.goto(`${baseURL}/browse`);
	await wallpaperRendered(device);
	await device.screenshot({ path: resolve(output, '01-alice-device-one.png') });
	checks.push(
		'Alice saved a local PNG through Settings; PUT returned 200 and the workspace rendered contain + frosted glass.'
	);
	const fresh = await second.newPage();
	observe(fresh, 'fresh-browser');
	await fresh.goto(baseURL);
	await fresh.getByLabel('Username', { exact: true }).waitFor();
	assert.deepEqual(await fresh.evaluate(() => Object.keys(localStorage)), []);
	const download = fresh.waitForResponse(
		(response) => new URL(response.url()).pathname === '/api/v1/settings/wallpaper/image'
	);
	await login(fresh, 'alice');
	const downloaded = await download;
	assert.equal(downloaded.status(), 200);
	assert.equal(downloaded.headers()['cache-control'], 'private, no-store');
	await wallpaperRendered(fresh);
	const restoredSource = await fresh.evaluate(() => {
		const settings: unknown = JSON.parse(localStorage.getItem('boxbox_settings:alice') ?? 'null');
		if (
			typeof settings !== 'object' ||
			settings === null ||
			!('backgroundImage' in settings) ||
			typeof settings.backgroundImage !== 'string'
		)
			throw new Error('Restored wallpaper missing from account preferences');
		return settings.backgroundImage;
	});
	const restoredId = getLocalWallpaperId(restoredSource);
	assert.equal(restoredId, `${digest}.${encodeURIComponent('alice')}`);
	const cachedHash = await fresh.evaluate(async (id) => {
		const database = await new Promise<IDBDatabase>((resolve, reject) => {
			const request = indexedDB.open('boxbox-wallpapers', 1);
			request.onsuccess = () => resolve(request.result);
			request.onerror = () => reject(request.error);
		});
		try {
			const blob = await new Promise<Blob>((resolve, reject) => {
				const request = database.transaction('wallpapers').objectStore('wallpapers').get(id);
				request.onsuccess = () => {
					const record: unknown = request.result;
					if (
						typeof record !== 'object' ||
						record === null ||
						!('id' in record) ||
						record.id !== id ||
						!('blob' in record) ||
						!(record.blob instanceof Blob)
					) {
						reject(new Error('Downloaded wallpaper missing from IndexedDB'));
						return;
					}
					resolve(record.blob);
				};
				request.onerror = () => reject(request.error);
			});
			return Array.from(
				new Uint8Array(await crypto.subtle.digest('SHA-256', await blob.arrayBuffer())),
				(byte) => byte.toString(16).padStart(2, '0')
			).join('');
		} finally {
			database.close();
		}
	}, restoredId);
	assert.equal(cachedHash, digest);
	await fresh.screenshot({ path: resolve(output, '02-alice-fresh-browser.png') });
	checks.push(
		'Alice logged in with empty localStorage and IndexedDB; downloaded PNG SHA-256 matched the upload and wallpaper + display settings rendered.'
	);
	const revalidation = fresh.waitForResponse(
		(response) => new URL(response.url()).pathname === '/api/v1/settings/wallpaper'
	);
	await fresh.reload();
	await revalidation;
	await wallpaperRendered(fresh);
	assert.equal(
		requests.filter(
			(request) => request.client === 'fresh-browser' && request.path.endsWith('/wallpaper/image')
		).length,
		1
	);
	checks.push(
		'Reload restored the session, revalidated metadata and reused IndexedDB bytes without another image download.'
	);
	await logout(fresh);
	checks.push('Logout removed the rendered wallpaper.');
	const noWallpaper = fresh.waitForResponse(
		(response) =>
			new URL(response.url()).pathname === '/api/v1/settings/wallpaper' && response.status() === 404
	);
	await login(fresh, 'bob');
	await noWallpaper;
	assert.equal(await fresh.locator('.app-personal-background').count(), 0);
	await fresh.screenshot({ path: resolve(output, '03-bob-shared-browser.png') });
	checks.push(
		'Bob used Alice’s browser after logout; his wallpaper returned 404 and his workspace stayed plain.'
	);
	await logout(fresh);
	const returning = fresh.waitForResponse(
		(response) => new URL(response.url()).pathname === '/api/v1/settings/wallpaper'
	);
	await login(fresh, 'alice');
	await returning;
	await wallpaperRendered(fresh);
	await fresh.screenshot({ path: resolve(output, '04-alice-return.png') });
	checks.push(
		'Alice returned to the shared browser and recovered her wallpaper and display settings.'
	);
	assert.deepEqual(failures, []);
	checks.push('No uncaught browser page errors.');
	const report = {
		baseURL,
		browser: browser.version(),
		sha256: digest,
		restoredId,
		checks,
		requests,
		pageErrors: failures
	};
	writeFileSync(resolve(output, 'browser-results.json'), `${JSON.stringify(report, null, 2)}\n`);
	console.log(JSON.stringify(report, null, 2));
} finally {
	await browser.close();
}
