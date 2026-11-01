import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, renameSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { chromium, type Request } from 'playwright';
import { startServer } from '../../../desktop/src/server';
import { array, record } from '../../src/lib/api/parse';
import { startServer as startWebServer } from './server';

const job = (value: unknown) => record(array(record(value, 'jobs').jobs, 'jobs')[0], 'job');

const root = mkdtempSync(join(tmpdir(), 'boxbox-review182-'));
const state = (delay: number, jobState = 'running', progress = 0, names = ['seed.txt']) => {
	writeFileSync(
		join(root, 'state-next.json'),
		JSON.stringify({ delay, jobState, progress, names, root: 'shared' })
	);
	renameSync(join(root, 'state-next.json'), join(root, 'state.json'));
};
state(0);
const fixtureBinary = join(root, 'tls-fixture');
const build = Bun.spawnSync([
	'go',
	'build',
	'-o',
	fixtureBinary,
	'frontend/scripts/perf/fixtures/slow-jobs.go'
]);
assert.equal(build.exitCode, 0, build.stderr.toString());
const fixture = Bun.spawn([fixtureBinary, root], {
	stdout: 'pipe',
	stderr: 'inherit'
});
const reader = fixture.stdout.getReader();
let output = '';
while (!output.includes('\n')) {
	const { value, done } = await reader.read();
	if (done) throw Error('fixture exited');
	output += new TextDecoder().decode(value);
}
const previousCA = process.env.SSL_CERT_FILE;
process.env.SSL_CERT_FILE = join(root, 'ca.pem');
const server = await startServer(join(process.cwd(), 'dist/boxbox'), join(root, 'state'));
if (previousCA === undefined) delete process.env.SSL_CERT_FILE;
else process.env.SSL_CERT_FILE = previousCA;
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext();
try {
	const page = await context.newPage();
	const errors: string[] = [];
	page.on('pageerror', (e) => errors.push(e.message));
	await page.goto(server.url);
	const origin = new URL(page.url()).origin;
	const paired: unknown = await page.evaluate(async (url) => {
		const r = await fetch('/api/v1/desktop/servers', {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({
				name: 'Slow TLS',
				url,
				code: 'ABCD-EFGH',
				clientName: 'Regression proof'
			})
		});
		if (!r.ok) throw Error(`pair ${r.status}`);
		return r.json();
	}, output.trim());
	assert.ok(
		typeof paired === 'object' && paired !== null && 'id' in paired && typeof paired.id === 'string'
	);
	const remote = `@remote/${paired.id}/shared`;
	let watchPolls = false;
	const activeJobRequests = new Set<Request>();
	let maxActiveJobRequests = 0;
	const isRemoteJobs = (url: string) => url.endsWith(`/desktop/servers/${paired.id}/api/v1/jobs`);
	page.on('request', (request) => {
		if (watchPolls && isRemoteJobs(request.url())) {
			activeJobRequests.add(request);
			maxActiveJobRequests = Math.max(maxActiveJobRequests, activeJobRequests.size);
		}
	});
	page.on('requestfinished', (request) => activeJobRequests.delete(request));
	page.on('requestfailed', (request) => activeJobRequests.delete(request));
	let listings = 0;
	page.on('request', (request) => {
		if (
			new URL(request.url()).pathname.endsWith(
				`/desktop/servers/${paired.id}/api/v1/files/list/shared`
			)
		)
			listings++;
	});
	const firstJobs = page.waitForResponse((r) =>
		r.url().endsWith(`/desktop/servers/${paired.id}/api/v1/jobs`)
	);
	await page.goto(`${origin}/browse?${new URLSearchParams({ path: remote })}`);
	await firstJobs;
	await page.getByText('seed.txt', { exact: true }).waitFor();
	watchPolls = true;
	state(3000, 'running', 50);
	const progress = await page.waitForResponse((r) =>
		r.url().endsWith(`/desktop/servers/${paired.id}/api/v1/jobs`)
	);
	assert.equal(job(await progress.json()).progress, 50);
	console.log('STEP verified TLS job response carries progress 50 after 3000ms');
	state(3000, 'completed', 100, ['seed.txt', 'completion.txt']);
	const terminal = await page.waitForResponse(
		async (r) =>
			r.url().endsWith(`/desktop/servers/${paired.id}/api/v1/jobs`) &&
			job(await r.json()).state === 'completed'
	);
	assert.equal(job(await terminal.json()).state, 'completed');
	await page.getByText('completion.txt', { exact: true }).waitFor({ timeout: 2000 });
	assert.equal(listings, 2, 'completion invalidates exactly once before directory polling');
	await page.waitForResponse((r) => r.url().endsWith(`/desktop/servers/${paired.id}/api/v1/jobs`));
	await new Promise((resolve) => setTimeout(resolve, 100));
	assert.equal(listings, 2, 'repeated terminal snapshots do not invalidate again');
	assert.equal(maxActiveJobRequests, 1, 'slow remote job requests never overlap');
	console.log('PASS: 3000ms TLS job polling advances and completion refreshes the directory once');
	// Offline flag without disabling loopback transport. CDP can emulate navigator independently.
	const cdp = await context.newCDPSession(page);
	await cdp.send('Network.enable');
	await cdp.send('Network.overrideNetworkState', {
		offline: true,
		latency: 0,
		downloadThroughput: -1,
		uploadThroughput: -1
	});
	assert.equal(await page.evaluate(() => navigator.onLine), false);
	assert.equal(await page.evaluate(async () => (await fetch('/health')).ok), true);
	mkdirSync(join(root, 'local', 'child'), { recursive: true });
	writeFileSync(join(root, 'local', 'seed.txt'), 'seed');
	writeFileSync(join(root, 'local', 'child', 'child.txt'), 'child');
	const path = `root${root}/local`;
	await page.goto(`${origin}/browse?${new URLSearchParams({ path })}`, {
		waitUntil: 'domcontentloaded'
	});
	assert.equal(await page.evaluate(() => navigator.onLine), false);
	await page.getByText('seed.txt', { exact: true }).waitFor({ timeout: 5000 });
	await page.getByText('child', { exact: true }).dblclick();
	await page.getByText('child.txt', { exact: true }).waitFor();
	const search = page.waitForResponse((r) => new URL(r.url()).pathname === '/api/v1/search');
	await page.getByRole('searchbox', { name: 'Search', exact: true }).fill('child');
	await search;
	await page.getByText('child.txt', { exact: true }).waitFor();
	const stats = page.waitForResponse((r) => new URL(r.url()).pathname === '/api/v1/files/stats');
	await page.goto(`${origin}/browse`, { waitUntil: 'domcontentloaded' });
	await stats;
	await page.getByText('root', { exact: true }).first().waitFor();
	console.log(
		'PASS: offline startup, local navigation, search and catalog with reachable loopback'
	);
	await page.goto(`${origin}/browse?${new URLSearchParams({ path: remote })}`, {
		waitUntil: 'domcontentloaded'
	});
	await assert.rejects(page.getByText('seed.txt', { exact: true }).waitFor({ timeout: 500 }));
	await cdp.send('Network.overrideNetworkState', {
		offline: false,
		latency: 0,
		downloadThroughput: -1,
		uploadThroughput: -1
	});
	await page.evaluate(() => window.dispatchEvent(new Event('online')));
	await page.getByText('seed.txt', { exact: true }).waitFor();
	console.log('PASS: qualified remote folder keeps its offline wait and resumes online');
	const web = await startWebServer(
		join(process.cwd(), 'dist/boxbox'),
		root,
		join(root, 'local'),
		'offline-web'
	);
	const webContext = await browser.newContext();
	try {
		await webContext.request.post(`${web.url}/api/v1/auth/login`, { data: web.credentials });
		const webPage = await webContext.newPage();
		const webCDP = await webContext.newCDPSession(webPage);
		await webCDP.send('Network.enable');
		await webCDP.send('Network.overrideNetworkState', {
			offline: true,
			latency: 0,
			downloadThroughput: -1,
			uploadThroughput: -1
		});
		await webPage.goto(`${web.url}/browse?path=perf`, { waitUntil: 'domcontentloaded' });
		assert.equal(await webPage.locator('meta[name="boxbox-desktop"]').count(), 0);
		assert.equal(await webPage.evaluate(() => navigator.onLine), false);
		assert.equal(await webPage.evaluate(async () => (await fetch('/health')).ok), true);
		await assert.rejects(webPage.getByText('seed.txt', { exact: true }).waitFor({ timeout: 500 }));
		await webCDP.send('Network.overrideNetworkState', {
			offline: false,
			latency: 0,
			downloadThroughput: -1,
			uploadThroughput: -1
		});
		await webPage.evaluate(() => window.dispatchEvent(new Event('online')));
		await webPage.getByText('seed.txt', { exact: true }).waitFor();
		console.log('PASS: normal web folder keeps its offline wait and resumes online');
	} finally {
		await webContext.close();
		await web.stop();
	}
	assert.deepEqual(errors, []);
} finally {
	await context.close();
	await browser.close();
	await server.stop();
	fixture.kill();
	await fixture.exited;
	rmSync(root, { recursive: true, force: true });
}
