import assert from 'node:assert/strict';
import type { Browser } from 'playwright';

/** Desktop APIs are isolated fixtures; the real renderer owns focus and keyboard handling. */
export async function verifyTrashSelection(
	browser: Browser,
	url: string,
	credentials: { username: string; password: string }
) {
	const context = await browser.newContext();
	try {
		const login = await context.request.post(`${url}/api/v1/auth/login`, { data: credentials });
		assert.equal(login.status(), 200);
		const page = await context.newPage();
		page.setDefaultTimeout(15_000);
		const errors: string[] = [];
		page.on('pageerror', (error) => errors.push(error.message));
		let deletes = 0;
		const item = {
			id: 'keyboard-fixture',
			name: 'Keyboard fixture.txt',
			originalPath: '/tmp/Keyboard fixture.txt',
			deletedAt: '2026-10-29T10:00:00Z',
			isDir: false,
			canRestore: true
		};
		await page.route('**/browse*', async (route) => {
			if (route.request().resourceType() !== 'document') return route.continue();
			const response = await route.fetch();
			await route.fulfill({
				response,
				body: (await response.text()).replace(
					'<head>',
					'<head><meta name="boxbox-desktop" content="true">'
				)
			});
		});
		await page.route('**/api/v1/desktop/**', async (route) => {
			const path = new URL(route.request().url()).pathname;
			if (path.endsWith('/events')) return route.abort();
			let json: unknown = {};
			if (path.endsWith('/trash')) json = { items: deletes ? [] : [item] };
			else if (path.endsWith('/trash/delete')) {
				deletes++;
				json = { ok: true };
			} else if (path.endsWith('/servers')) json = { servers: [] };
			else if (path.endsWith('/volumes')) json = { available: true, volumes: [] };
			await route.fulfill({ json });
		});
		await page.goto(`${url}/browse?page=trash`);
		const checkbox = page.getByRole('checkbox', { name: `Select ${item.name}`, exact: true });
		await checkbox.check();
		await checkbox.press('Delete');
		await page.getByRole('alert').filter({ hasText: 'cannot be undone' }).waitFor();
		assert.equal(deletes, 0, 'first Delete only requests confirmation');
		await checkbox.press('Space');
		assert.equal(await checkbox.isChecked(), false, 'Space still unchecks the focused checkbox');
		assert.equal(await page.getByRole('alert').filter({ hasText: 'cannot be undone' }).count(), 0);
		await checkbox.press('Delete');
		assert.equal(deletes, 0, 'Delete on an unselected checkbox is harmless');
		await checkbox.press('Space');
		assert.equal(await checkbox.isChecked(), true);
		await checkbox.press('Delete');
		await page.getByRole('button', { name: 'Confirm delete', exact: true }).waitFor();
		await checkbox.press('Delete');
		await page.getByText('Trash is empty', { exact: true }).waitFor();
		assert.equal(deletes, 1, 'second Delete performs only the confirmed fixture mutation');
		assert.deepEqual(errors, []);
		console.log('PASS: Trash checkbox Delete, Space, deselection, and two-step confirmation');
	} finally {
		await context.close();
	}
}
