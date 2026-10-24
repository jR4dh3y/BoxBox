import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Browser, Page } from 'playwright';
import { repo } from './common';

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function chunkPath(source: string): string {
	const manifest: unknown = JSON.parse(
		readFileSync(join(repo, 'frontend/.svelte-kit/output/client/.vite/manifest.json'), 'utf8')
	);
	assert.ok(isRecord(manifest));
	const chunk = manifest[source];
	assert.ok(isRecord(chunk) && typeof chunk.file === 'string', `Missing chunk for ${source}`);
	return chunk.file;
}

type LoadCase = {
	label: string;
	source: string;
	path: string;
	activate?: (page: Page) => Promise<void>;
	ready: (page: Page) => Promise<void>;
};

const cases: LoadCase[] = [
	{
		label: 'grid view',
		source: 'src/lib/components/FileGrid.svelte',
		path: '/browse?path=perf&view=grid',
		ready: (page) => page.locator('[data-file-item="true"]').first().waitFor()
	},
	{
		label: 'wallpaper settings',
		source: 'src/lib/components/settings/wallpaper/WallpaperSettings.svelte',
		path: '/settings',
		ready: (page) => page.getByRole('button', { name: 'Choose Wallpaper', exact: true }).waitFor()
	},
	{
		label: 'shared links',
		source: 'src/lib/components/settings/ShareLinksSettings.svelte',
		path: '/settings',
		ready: (page) => page.getByText('No shared links.', { exact: true }).waitFor()
	},
	{
		label: 'location editor',
		source: 'src/lib/components/PathEditor.svelte',
		path: '/browse?path=perf',
		activate: async (page) => {
			await page.getByText('File-00000.txt', { exact: true }).waitFor();
			await page.keyboard.press('Control+l');
		},
		ready: (page) => page.getByRole('combobox', { name: 'Location', exact: true }).waitFor()
	},
	{
		label: 'file menu',
		source: 'src/lib/components/FileContextMenu.svelte',
		path: '/browse?path=perf',
		activate: async (page) => {
			await page.getByText('File-00000.txt', { exact: true }).click({ button: 'right' });
		},
		ready: (page) => page.getByRole('menu').waitFor()
	},
	{
		label: 'drives',
		source: 'src/lib/components/DriveCard.svelte',
		path: '/browse',
		ready: (page) => page.locator('button span[title="perf"]').waitFor()
	}
];

export async function verifyLazyLoadErrors(
	browser: Browser,
	url: string,
	credentials: { username: string; password: string }
) {
	for (const test of cases) {
		const context = await browser.newContext();
		try {
			const login = await context.request.post(`${url}/api/v1/auth/login`, { data: credentials });
			assert.equal(login.ok(), true);
			const blockedPath = `**/${chunkPath(test.source)}`;
			let blockedRequests = 0;
			await context.route(blockedPath, (route) => {
				blockedRequests++;
				return route.abort('failed');
			});
			const page = await context.newPage();
			const errors: string[] = [];
			page.on('pageerror', (error) => errors.push(error.message));
			await page.goto(`${url}${test.path}`);
			if (test.activate) {
				assert.equal(blockedRequests, 0, `${test.label}: do not load before interaction`);
				await test.activate(page);
			}
			const alert = page
				.getByRole('alert')
				.filter({ hasText: `Could not load the ${test.label}.` });
			await alert.waitFor({ timeout: 5000 });
			assert.ok(blockedRequests > 0, `${test.label}: the component request must be blocked`);
			await context.unroute(blockedPath);
			await alert.getByRole('button', { name: 'Reload', exact: true }).click();
			await test.activate?.(page);
			await test.ready(page);
			assert.equal(page.url(), `${url}${test.path}`, 'reload preserves the current location');
			assert.equal(await alert.count(), 0, 'successful recovery removes the error');
			assert.deepEqual(errors, [], 'component failures are handled without uncaught errors');
		} finally {
			await context.close();
		}
	}
	console.log(
		'PASS: failed grid, wallpaper, shared-link, location editor, file menu and drive chunks recover on reload'
	);
}
