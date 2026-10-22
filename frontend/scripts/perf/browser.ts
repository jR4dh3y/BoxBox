import assert from 'node:assert/strict';
import { join } from 'node:path';
import { writeFileSync } from 'node:fs';
import { chromium, type Browser, type Page } from 'playwright';
import budget from './budget.json';
import bundleBudget from '../../perf-budget.json';
import { browserViolations, median, type BrowserMetrics } from './gates';
import { baseArgument, repo, results, run, withBaseline } from './common';
import { createDirectoryFixture, startServer } from './server';
import { verifySelectionRefresh } from './selection-refresh';
import { verifyNavigation } from './navigation';
import { verifyPasteLoadRace } from './paste-load-race';
import { verifyLazyLoadErrors } from './lazy-load-errors';

const profiles = [
	{ name: 'local', latency: 0, downloadThroughput: -1, uploadThroughput: -1 },
	{ name: 'slow', latency: 150, downloadThroughput: 200_000, uploadThroughput: 93_750 }
];
type Sample = BrowserMetrics & {
	variant: string;
	profile: string;
	cache: 'cold' | 'warm';
	filesVisibleMs: number;
	scriptTransferredBytes: number;
};
const samples: Sample[] = [];

function recordRequests(page: Page) {
	const requests: { path: string; method: string }[] = [];
	page.on('request', (request) => {
		requests.push({ path: new URL(request.url()).pathname, method: request.method() });
	});
	return requests;
}

async function measure(
	browser: Browser,
	server: Awaited<ReturnType<typeof startServer>>,
	variant: string,
	profile: (typeof profiles)[number]
) {
	const { url, credentials } = server;
	const context = await browser.newContext({ viewport: { width: 1440, height: 1200 } });
	const page = await context.newPage();
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	const requests = recordRequests(page);
	try {
		const login = await context.request.post(`${url}/api/v1/auth/login`, { data: credentials });
		assert.equal(login.ok(), true, 'fixture login establishes a real HttpOnly refresh cookie');
		const cdp = await context.newCDPSession(page);
		await cdp.send('Network.enable');
		await cdp.send('Network.emulateNetworkConditions', {
			offline: false,
			latency: profile.latency,
			downloadThroughput: profile.downloadThroughput,
			uploadThroughput: profile.uploadThroughput
		});
		await page.addInitScript(() => {
			const observer = new MutationObserver(() => {
				const files = document.querySelectorAll('[data-file-row="true"], [data-file-item="true"]');
				if (!files[0]?.textContent?.includes('File-00000')) return;
				observer.disconnect();
				requestAnimationFrame(() =>
					requestAnimationFrame(() => performance.mark('boxbox:files-visible'))
				);
			});
			observer.observe(document, { subtree: true, childList: true });
		});
		for (const cache of ['cold', 'warm'] as const) {
			requests.length = 0;
			await page.goto(`${url}/browse?path=perf`, { waitUntil: 'commit', timeout: 60_000 });
			await page.waitForFunction(
				() => performance.getEntriesByName('boxbox:files-visible').length > 0,
				null,
				{ timeout: 60_000 }
			);
			// Allow startup revalidation to finish before counting requests; the
			// rendered marker, not network-idle, supplies the timing measurement.
			await page.waitForLoadState('networkidle');
			const renderedFiles = await page
				.locator('[data-file-row="true"], [data-file-item="true"]')
				.count();
			assert.ok(renderedFiles <= 150, 'a large folder must keep rendered file elements bounded');
			assert.equal(await page.getByText('File-00000.txt', { exact: true }).isVisible(), true);
			if (variant === 'head')
				assert.equal(await page.getByText('10000 items', { exact: true }).isVisible(), true);
			const timing = await page.evaluate(() => {
				const scripts = performance
					.getEntriesByType('resource')
					.filter(
						(entry): entry is PerformanceResourceTiming =>
							entry instanceof PerformanceResourceTiming &&
							new URL(entry.name).pathname.endsWith('.js')
					);
				return {
					filesVisibleMs: performance.getEntriesByName('boxbox:files-visible')[0].startTime,
					scriptBytes: scripts.reduce((total, entry) => total + entry.decodedBodySize, 0),
					scriptTransferredBytes: scripts.reduce(
						(total, entry) => total + (entry.transferSize === 0 ? 0 : entry.encodedBodySize),
						0
					),
					scriptRequests: scripts.length
				};
			});
			assert.ok(timing.scriptBytes > 0, 'JS byte accounting must not silently return zero');
			const sample: Sample = {
				variant,
				profile: profile.name,
				cache,
				...timing,
				listingRequests: requests.filter(
					(request) => request.path.startsWith('/api/v1/files/list/') && request.method === 'GET'
				).length,
				refreshRequests: requests.filter((request) => request.path === '/api/v1/auth/refresh')
					.length
			};
			samples.push(sample);
			if (variant === 'head') {
				const violations = browserViolations(sample, {
					...budget,
					scriptBytes: Math.min(budget.scriptBytes, bundleBudget.staticJsBytes)
				});
				assert.deepEqual(violations, [], `${profile.name}/${cache}: ${violations.join('; ')}`);
				assert.equal(
					sample.listingRequests,
					1,
					'the complete folder must come from one real listing request'
				);
				assert.equal(
					sample.refreshRequests,
					1,
					'each navigation restores its session exactly once'
				);
				assert.deepEqual(errors, [], 'no runtime errors during browsing');
			}
		}
	} catch (error) {
		await page.screenshot({ path: join(results, `${variant}-${profile.name}-failure.png`) });
		writeFileSync(
			join(results, `${variant}-${profile.name}-failure.json`),
			JSON.stringify({ url: page.url(), requests, errors }, null, 2)
		);
		throw error;
	} finally {
		await context.close();
	}
}

async function loggedOut(browser: Browser, url: string) {
	const context = await browser.newContext();
	try {
		const page = await context.newPage();
		const requests = recordRequests(page);
		const errors: string[] = [];
		page.on('pageerror', (error) => errors.push(error.message));
		for (const path of ['/browse?path=perf', '/browse']) {
			requests.length = 0;
			await page.goto(`${url}${path}`);
			await page.waitForURL('**/login**');
			assert.equal(await page.locator('input[type="password"]').isVisible(), true);
			await page.waitForLoadState('networkidle');
			const refreshes = requests.filter(
				(request) => request.path === '/api/v1/auth/refresh'
			).length;
			assert.equal(refreshes, 1, 'logged-out startup performs exactly one refresh');
			assert.equal(
				requests.filter((request) => request.path.startsWith('/api/v1/files')).length,
				0,
				'unauthenticated visits must not fetch private files or mounts'
			);
			assert.deepEqual(errors, []);
		}
	} finally {
		await context.close();
	}
}

function build(root: string) {
	if (root !== repo)
		run(
			['bun', 'install', '--frozen-lockfile'],
			join(root, 'frontend'),
			join(results, 'browser-base-install.log')
		);
	run(
		['bash', 'scripts/build-single.sh'],
		root,
		join(results, root === repo ? 'browser-head-build.log' : 'browser-base-build.log')
	);
}

async function benchmark(base?: string, sha?: string) {
	build(repo);
	if (base) build(base);
	const fixture = createDirectoryFixture();
	const servers: Awaited<ReturnType<typeof startServer>>[] = [];
	let browser: Browser | undefined;
	try {
		const head = await startServer(join(repo, 'dist/boxbox'), fixture.root, fixture.mount, 'head');
		servers.push(head);
		const previous = base
			? await startServer(join(base, 'dist/boxbox'), fixture.root, fixture.mount, 'base')
			: undefined;
		if (previous) servers.push(previous);
		browser = await chromium.launch({ headless: true });
		for (let repeat = 0; repeat < 5; repeat++) {
			for (const profile of profiles) {
				const variants = previous
					? [
							{ name: 'base', server: previous },
							{ name: 'head', server: head }
						]
					: [{ name: 'head', server: head }];
				if (repeat % 2) variants.reverse();
				for (const variant of variants)
					await measure(browser, variant.server, variant.name, profile);
			}
		}
		await loggedOut(browser, head.url);
		await verifyLazyLoadErrors(browser, head.url, head.credentials);
		await verifyNavigation(browser, head.url, head.credentials, fixture.mount);
		await verifySelectionRefresh(browser, head.url, head.credentials, fixture.mount);
	} finally {
		writeFileSync(
			join(results, 'browser.json'),
			JSON.stringify({ base: sha ?? null, samples }, null, 2)
		);
		await browser?.close();
		await Promise.all(servers.map((server) => server.stop()));
		fixture.dispose();
	}
	const summaries: string[] = [
		'Files-visible medians (advisory; wall-clock time is not a merge gate):'
	];
	for (const profile of profiles) {
		for (const cache of ['cold', 'warm']) {
			for (const variant of base ? ['base', 'head'] : ['head']) {
				const values = samples
					.filter(
						(sample) =>
							sample.profile === profile.name &&
							sample.cache === cache &&
							sample.variant === variant
					)
					.map((sample) => sample.filesVisibleMs);
				summaries.push(
					`${variant} ${profile.name}/${cache}: ${median(values).toFixed(0)} ms (n=${values.length})`
				);
			}
		}
	}
	writeFileSync(join(results, 'browser-summary.txt'), summaries.join('\n'));
	console.log(summaries.join('\n'));
}

const ref = baseArgument();
if (ref) await withBaseline(ref, benchmark);
else await benchmark();
