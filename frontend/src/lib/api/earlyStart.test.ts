import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { afterEach, beforeEach, describe, test } from 'node:test';
import { runInNewContext } from 'node:vm';
import { createEarlyStart } from './earlyStart';
import { restoreSession } from './auth';
import { directoryUrl, type FileList } from './files';
import { tokenStorage } from '$lib/utils/storage';

const script = readFileSync(new URL('../../../static/early-fetch.js', import.meta.url), 'utf8');
const origin = 'http://localhost';
const tokens = { accessToken: 'early-token', expiresAt: '2030-01-01T00:00:00Z' };
const listing: FileList = { path: 'files/C', items: [], totalCount: 0, page: 1, pageSize: 50 };

type Win = Pick<Window, 'boxboxEarly'>;
interface Call {
	url: string;
	method: string;
	authorization: string | undefined;
}

interface Page {
	href?: string;
	settings?: string | null;
	isDevelopment?: boolean;
	refresh?: () => Response | Promise<Response>;
	list?: () => Response | Promise<Response>;
}

/** Runs static/early-fetch.js in a page with a recording `fetch`. */
function runShell(page: Page, win: Win = {}) {
	const calls: Call[] = [];
	runInNewContext(script, {
		window: win,
		location: new URL(page.href ?? `${origin}/browse?path=files/C`),
		localStorage: { getItem: () => page.settings ?? null },
		document: {
			querySelector: () => (page.isDevelopment ? { getAttribute: () => 'true' } : null)
		},
		URL,
		fetch: async (
			input: URL | string,
			init?: { method?: string; headers?: Record<string, string> }
		) => {
			const url = String(input);
			calls.push({
				url,
				method: init?.method ?? 'GET',
				authorization: init?.headers?.Authorization
			});
			if (url.endsWith('/auth/refresh')) return (page.refresh ?? (() => Response.json(tokens)))();
			return (page.list ?? (() => Response.json(listing)))();
		}
	});
	return { win, calls };
}

const refreshCalls = (calls: Call[]) => calls.filter((call) => call.url.endsWith('/auth/refresh'));
const listCalls = (calls: Call[]) => calls.filter((call) => call.url.includes('/files/list/'));

const realFetch = globalThis.fetch;
const realWindow = globalThis.window;
const realLocalStorage = globalThis.localStorage;
beforeEach(() =>
	Object.assign(globalThis, {
		window: { location: { origin } },
		localStorage: { removeItem: () => {} }
	})
);
afterEach(() => {
	Object.assign(globalThis, {
		fetch: realFetch,
		window: realWindow,
		localStorage: realLocalStorage
	});
	tokenStorage.clearTokens();
});

describe('the shell script asks for the listing the app would ask for', () => {
	// Each case gives the page state and the options the app derives from it; both sides must agree.
	const cases = [
		{
			name: 'defaults',
			search: '?path=files/C',
			path: 'files/C',
			options: { sortBy: 'name', sortDir: 'asc', includeHidden: false }
		},
		{
			name: 'sort and direction from the address',
			search: '?path=files/C&sort=size&dir=desc',
			path: 'files/C',
			options: { sortBy: 'size', sortDir: 'desc', includeHidden: false }
		},
		{
			name: 'unknown sort and direction fall back to the saved defaults',
			search: '?path=files&sort=bogus&dir=sideways',
			settings: JSON.stringify({ defaultSortBy: 'modTime', defaultSortDir: 'desc' }),
			path: 'files',
			options: { sortBy: 'modTime', sortDir: 'desc', includeHidden: false }
		},
		{
			name: 'show hidden files',
			search: '?path=files/C',
			settings: JSON.stringify({ showHiddenFiles: true }),
			path: 'files/C',
			options: { sortBy: 'name', sortDir: 'asc', includeHidden: true }
		},
		{
			name: 'spaces, accents and surrounding slashes in the path',
			search: '?path=%2Ffiles%2FMy%20Photos%2F%C3%A9t%C3%A9%2F',
			path: 'files/My Photos/été',
			options: { sortBy: 'name', sortDir: 'asc', includeHidden: false }
		},
		{
			name: 'unreadable saved settings',
			search: '?path=files/C',
			settings: '{not json',
			path: 'files/C',
			options: { sortBy: 'name', sortDir: 'asc', includeHidden: false }
		}
	] as const;

	for (const { name, search, path, options, ...rest } of cases) {
		test(name, () => {
			const { win } = runShell({
				href: `${origin}/browse${search}`,
				settings: 'settings' in rest ? rest.settings : null,
				isDevelopment: true
			});
			const expected = directoryUrl(path, { ...options, page: 1, pageSize: 50 });
			assert.equal(win.boxboxEarly?.list.url, expected);
		});
	}
});

describe('the shell script', () => {
	test('refreshes the session once, then lists with the new token', async () => {
		const { win, calls } = runShell({});
		await win.boxboxEarly?.list.response;
		assert.deepEqual(
			calls.map((call) => [call.method, call.url.replace(origin, ''), call.authorization]),
			[
				['POST', '/api/v1/auth/refresh', undefined],
				[
					'GET',
					'/api/v1/files/list/files/C?page=1&pageSize=50&sortBy=name&sortDir=asc&includeHidden=false',
					'Bearer early-token'
				]
			]
		);
	});

	test('a development server has no authentication: it lists at once and never refreshes', async () => {
		const { win, calls } = runShell({ isDevelopment: true });
		await win.boxboxEarly?.list.response;
		assert.equal(win.boxboxEarly?.session, null);
		assert.equal(refreshCalls(calls).length, 0);
		assert.deepEqual(
			listCalls(calls).map((call) => call.authorization),
			[undefined]
		);
	});

	test('a refused refresh means no listing is requested', async () => {
		const { win, calls } = runShell({ refresh: () => new Response('no', { status: 401 }) });
		assert.equal(await win.boxboxEarly?.list.response, null);
		assert.equal(await win.boxboxEarly?.session, null);
		assert.equal(listCalls(calls).length, 0);
	});

	test('a failed listing request resolves to nothing instead of an error', async () => {
		const { win } = runShell({ list: () => new Response('boom', { status: 500 }) });
		assert.equal(await win.boxboxEarly?.list.response, null);
	});

	test('starts nothing outside a folder of the browse page', () => {
		for (const href of [
			`${origin}/browse`,
			`${origin}/browse?path=`,
			`${origin}/settings?path=files`,
			`${origin}/login`
		]) {
			const { win, calls } = runShell({ href });
			assert.equal(win.boxboxEarly, undefined, href);
			assert.equal(calls.length, 0, href);
		}
	});

	test('stands down when the app already took over, so only one refresh can happen', () => {
		const { calls } = runShell({}, { boxboxEarly: null });
		assert.equal(calls.length, 0);
	});

	test('stands down when it already ran', () => {
		const first = runShell({});
		const second = runShell({}, first.win);
		assert.equal(refreshCalls(first.calls).length, 1);
		assert.equal(second.calls.length, 0);
	});
});

describe('the app takes over what the shell started', () => {
	test('end to end: one refresh and one listing serve the whole page load', async () => {
		const { win, calls } = runShell({});
		const early = createEarlyStart(() => win);

		const session = await early.takeSession();
		const first = await early.takeDirectory(
			directoryUrl('files/C', {
				page: 1,
				pageSize: 50,
				sortBy: 'name',
				sortDir: 'asc',
				includeHidden: false
			})
		);

		assert.deepEqual(session, tokens);
		assert.deepEqual(first, listing);
		assert.equal(refreshCalls(calls).length, 1);
		assert.equal(listCalls(calls).length, 1);
	});

	test('each take happens once, and the handoff is closed afterwards', async () => {
		const { win } = runShell({});
		const early = createEarlyStart(() => win);
		const url = win.boxboxEarly?.list.url ?? '';

		assert.deepEqual(await early.takeSession(), tokens);
		assert.equal(win.boxboxEarly, null);
		assert.equal(await early.takeSession(), null);
		assert.deepEqual(await early.takeDirectory(url), listing);
		assert.equal(await early.takeDirectory(url), null);
	});

	test('a script that arrives after the app took over starts nothing', async () => {
		const win: Win = {};
		const early = createEarlyStart(() => win);
		assert.equal(await early.takeSession(), null);

		const { calls } = runShell({}, win);
		assert.equal(calls.length, 0);
	});

	test('a listing for a different request is dropped, not used', async () => {
		const { win } = runShell({});
		const early = createEarlyStart(() => win);
		const other = directoryUrl('files/D', {
			page: 1,
			pageSize: 50,
			sortBy: 'name',
			sortDir: 'asc',
			includeHidden: false
		});
		assert.equal(await early.takeDirectory(other), null);
		assert.equal(await early.takeDirectory(win.boxboxEarly?.list.url ?? ''), null);
	});

	test('a listing older than the limit is not shown as current', async () => {
		const { win } = runShell({});
		const url = win.boxboxEarly?.list.url ?? '';
		const handoff = win.boxboxEarly;
		if (!handoff) throw new Error('the shell script did not start');
		handoff.list.startedAt = Date.now() - 60_000;
		assert.equal(await createEarlyStart(() => win).takeDirectory(url), null);
	});

	test('a reply that is not the listing JSON is dropped', async () => {
		const { win } = runShell({
			list: () => new Response('<html></html>', { headers: { 'Content-Type': 'text/html' } })
		});
		const url = win.boxboxEarly?.list.url ?? '';
		assert.equal(await createEarlyStart(() => win).takeDirectory(url), null);
	});

	test('a session reply without a token is not used', async () => {
		const { win } = runShell({ refresh: () => Response.json({ accessToken: 7 }) });
		assert.equal(await createEarlyStart(() => win).takeSession(), null);
	});
});

describe('restoreSession', () => {
	let requests: string[] = [];
	beforeEach(() => {
		requests = [];
		Object.assign(globalThis, {
			fetch: async (input: URL | string) => {
				requests.push(String(input));
				return Response.json({ accessToken: 'app-token', expiresAt: tokens.expiresAt });
			}
		});
	});

	test('uses the refresh the shell started and asks the server for nothing more', async () => {
		const { win } = runShell({});
		const early = createEarlyStart(() => win);
		const restored = await restoreSession(early);
		assert.equal(restored.accessToken, 'early-token');
		assert.equal(tokenStorage.getAccessToken(), 'early-token');
		assert.deepEqual(requests, []);
	});

	test('waits for a refresh that is still in flight instead of starting a second one', async () => {
		let answer: (response: Response) => void = () => {};
		const { win, calls } = runShell({
			refresh: () => new Promise<Response>((resolve) => (answer = resolve))
		});
		const restoring = restoreSession(createEarlyStart(() => win));

		await new Promise((resolve) => setTimeout(resolve, 10));
		assert.equal(
			tokenStorage.getAccessToken(),
			null,
			'nothing is stored before the server answers'
		);
		answer(Response.json(tokens));
		assert.equal((await restoring).accessToken, 'early-token');
		assert.equal(refreshCalls(calls).length, 1);
		assert.deepEqual(requests, []);
	});

	test('refreshes by itself when the shell started nothing or its refresh failed', async () => {
		const failed = runShell({ refresh: () => new Response('no', { status: 401 }) });
		assert.equal(
			(await restoreSession(createEarlyStart(() => failed.win))).accessToken,
			'app-token'
		);
		assert.equal(requests.length, 1);
		assert.match(requests[0], /\/auth\/refresh$/);

		requests = [];
		assert.equal((await restoreSession(createEarlyStart(() => ({})))).accessToken, 'app-token');
		assert.equal(requests.length, 1);
	});
});
