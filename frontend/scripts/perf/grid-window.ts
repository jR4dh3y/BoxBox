import assert from 'node:assert/strict';
import type { Browser } from 'playwright';

export async function verifyGridWindow(
	browser: Browser,
	url: string,
	credentials: { username: string; password: string }
) {
	const context = await browser.newContext({ viewport: { width: 1440, height: 1200 } });
	try {
		await context.request.post(`${url}/api/v1/auth/login`, { data: credentials });
		const page = await context.newPage();
		let listings = 0;
		page.on('request', (request) => {
			if (new URL(request.url()).pathname.startsWith('/api/v1/files/list/')) listings++;
		});
		await page.goto(`${url}/browse?path=perf&view=grid`);
		const tiles = page.locator('[data-file-item="true"]');
		await tiles.first().waitFor();
		const counts: number[] = [];
		for (const position of [0, 0.5, 1]) {
			await tiles.first().evaluate((tile, position) => {
				const scroller = tile.parentElement?.parentElement;
				if (!scroller) throw new Error('Missing grid scroller');
				scroller.scrollTop = (scroller.scrollHeight - scroller.clientHeight) * position;
			}, position);
			await page.evaluate(
				() =>
					new Promise<void>((resolve) =>
						requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
					)
			);
			const count = await tiles.count();
			counts.push(count);
			assert.ok(count <= 150, `grid rendered ${count} tiles at ${position}`);
		}
		await page.getByText('File-09999.txt', { exact: true }).waitFor();
		assert.equal(listings, 1, 'scrolling never fetches another listing');
		console.log(
			`PASS: 1440×1200 grid renders ${counts.join('/')} tiles at top/middle/bottom with one listing`
		);
	} finally {
		await context.close();
	}
}
