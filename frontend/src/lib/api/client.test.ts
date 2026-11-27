import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
	ApiRequestError,
	AuthSessionClosedError,
	AuthRefreshEvent,
	apiRequest,
	clearTokens,
	getAccessToken,
	logoutSession,
	refreshSession,
	setAccessToken
} from './client';
import { refresh } from './auth';

function installWindow(origin: string) {
	const previousWindow = globalThis.window;
	const previousLocalStorage = globalThis.localStorage;
	const values = new Map<string, string>();
	const activeWindow = Object.assign(new EventTarget(), { location: { origin } });
	const activeStorage = {
		getItem(key: string) {
			return values.get(key) ?? null;
		},
		setItem(key: string, value: string) {
			values.set(key, value);
		},
		removeItem(key: string) {
			values.delete(key);
		}
	};
	Object.assign(globalThis, { window: activeWindow, localStorage: activeStorage });
	return { activeWindow, previousWindow, previousLocalStorage };
}

describe('auth persistence in the API client', () => {
	test('an already cancelled mutation never reaches fetch', async () => {
		const originalFetch = globalThis.fetch;
		const controller = new AbortController();
		controller.abort();
		let requestCount = 0;
		try {
			globalThis.fetch = Object.assign(
				async () => {
					requestCount++;
					return new Response();
				},
				{ preconnect: originalFetch.preconnect }
			);
			await assert.rejects(
				apiRequest('/files/test', { method: 'DELETE', signal: controller.signal }),
				{ name: 'AbortError' }
			);
			assert.equal(requestCount, 0);
		} finally {
			globalThis.fetch = originalFetch;
		}
	});

	test('automatic and explicit callers share one in-flight refresh', async () => {
		let beginRefresh: () => void = () => {
			throw new Error('Refresh request did not start');
		};
		const refreshStarted = new Promise<void>((resolve) => {
			beginRefresh = resolve;
		});
		let releaseRefresh: () => void = () => {
			throw new Error('Refresh gate was not initialized');
		};
		const refreshGate = new Promise<void>((resolve) => {
			releaseRefresh = resolve;
		});
		let refreshCalls = 0;
		let protectedCalls = 0;
		const server = Bun.serve({
			port: 0,
			async fetch(request) {
				const { pathname } = new URL(request.url);
				if (pathname === '/api/v1/auth/refresh') {
					refreshCalls += 1;
					beginRefresh();
					await refreshGate;
					return Response.json({ accessToken: 'rotated-token', expiresAt: '2030-01-01T00:00:00Z' });
				}
				if (pathname === '/api/v1/protected') {
					protectedCalls += 1;
					if (protectedCalls === 1) {
						return Response.json({ error: 'Unauthorized', code: 'TOKEN_INVALID' }, { status: 401 });
					}
					return Response.json({ value: 'available' });
				}
				return Response.json({ error: 'Not found', code: 'NOT_FOUND' }, { status: 404 });
			}
		});
		const { previousWindow, previousLocalStorage } = installWindow(server.url.origin);
		clearTokens();
		setAccessToken('original-token');

		try {
			const automaticRequest = apiRequest<{ value: string }>('/protected');
			await refreshStarted;
			const firstExplicitRefresh = refresh();
			const secondExplicitRefresh = refreshSession();
			assert.equal(refreshCalls, 1);
			releaseRefresh();
			assert.deepEqual(await automaticRequest, { value: 'available' });
			assert.equal((await firstExplicitRefresh).accessToken, 'rotated-token');
			assert.equal((await secondExplicitRefresh).accessToken, 'rotated-token');
			assert.equal(refreshCalls, 1);
			assert.equal(protectedCalls, 2);
			assert.equal(getAccessToken(), 'rotated-token');
		} finally {
			releaseRefresh();
			clearTokens();
			Object.assign(globalThis, { window: previousWindow, localStorage: previousLocalStorage });
			server.stop(true);
		}
	});

	test('a transient refresh failure preserves the token and can be retried', async () => {
		let refreshCalls = 0;
		let protectedCalls = 0;
		const server = Bun.serve({
			port: 0,
			fetch(request) {
				const { pathname } = new URL(request.url);
				if (pathname === '/api/v1/auth/refresh') {
					refreshCalls += 1;
					if (refreshCalls === 1) {
						return Response.json(
							{ error: 'Storage unavailable', code: 'INTERNAL_ERROR' },
							{ status: 500 }
						);
					}
					return Response.json({
						accessToken: 'recovered-token',
						expiresAt: '2030-01-01T00:00:00Z'
					});
				}
				if (pathname === '/api/v1/protected') {
					protectedCalls += 1;
					return Response.json({ error: 'Unauthorized', code: 'TOKEN_INVALID' }, { status: 401 });
				}
				return Response.json({ error: 'Not found', code: 'NOT_FOUND' }, { status: 404 });
			}
		});
		const { previousWindow, previousLocalStorage } = installWindow(server.url.origin);
		clearTokens();
		setAccessToken('original-token');

		try {
			await assert.rejects(apiRequest('/protected'), {
				name: 'ApiRequestError',
				status: 500,
				code: 'INTERNAL_ERROR'
			});
			assert.equal(getAccessToken(), 'original-token');
			assert.equal(protectedCalls, 1);

			const response = await refresh();
			assert.equal(response.accessToken, 'recovered-token');
			assert.equal(getAccessToken(), 'recovered-token');
			assert.equal(refreshCalls, 2);
		} finally {
			clearTokens();
			Object.assign(globalThis, { window: previousWindow, localStorage: previousLocalStorage });
			server.stop(true);
		}
	});
	test('a network refresh failure preserves the current token', async () => {
		const server = Bun.serve({
			port: 0,
			fetch() {
				return Response.json({ error: 'Not found', code: 'NOT_FOUND' }, { status: 404 });
			}
		});
		const origin = server.url.origin;
		server.stop(true);
		const { previousWindow, previousLocalStorage } = installWindow(origin);
		clearTokens();
		setAccessToken('original-token');

		try {
			await assert.rejects(refresh(), { name: 'TypeError' });
			assert.equal(getAccessToken(), 'original-token');
		} finally {
			clearTokens();
			Object.assign(globalThis, { window: previousWindow, localStorage: previousLocalStorage });
		}
	});

	test('a terminal recovery failure clears tokens and preserves its typed error', async () => {
		const server = Bun.serve({
			port: 0,
			fetch(request) {
				if (new URL(request.url).pathname === '/api/v1/auth/refresh') {
					return Response.json(
						{ error: 'Sign in required', code: 'SESSION_RECOVERY_REQUIRED' },
						{ status: 401 }
					);
				}
				return Response.json({ error: 'Not found', code: 'NOT_FOUND' }, { status: 404 });
			}
		});
		const { activeWindow, previousWindow, previousLocalStorage } = installWindow(server.url.origin);
		let refreshFailure: unknown = null;
		const captureRefreshFailure = (event: Event) => {
			if (event instanceof AuthRefreshEvent && event.result.kind === 'failed') {
				refreshFailure = event.result.error;
			}
		};
		activeWindow.addEventListener(AuthRefreshEvent.eventName, captureRefreshFailure);
		clearTokens();
		setAccessToken('original-token');

		try {
			await assert.rejects(refresh(), {
				name: 'ApiRequestError',
				status: 401,
				code: 'SESSION_RECOVERY_REQUIRED'
			});
			assert.equal(getAccessToken(), null);
			assert.equal(
				refreshFailure instanceof ApiRequestError ? refreshFailure.code : null,
				'SESSION_RECOVERY_REQUIRED'
			);
			await assert.rejects(refreshSession(), AuthSessionClosedError);
		} finally {
			activeWindow.removeEventListener(AuthRefreshEvent.eventName, captureRefreshFailure);
			clearTokens();
			Object.assign(globalThis, { window: previousWindow, localStorage: previousLocalStorage });
			server.stop(true);
		}
	});

	test('logout drains pending refresh and blocks refresh after completion', async () => {
		let beginRefresh: () => void = () => {
			throw new Error('Refresh request did not start');
		};
		const refreshStarted = new Promise<void>((resolve) => {
			beginRefresh = resolve;
		});
		let releaseRefresh: () => void = () => {
			throw new Error('Refresh gate was not initialized');
		};
		const refreshGate = new Promise<void>((resolve) => {
			releaseRefresh = resolve;
		});
		let refreshCalls = 0;
		let logoutCalls = 0;
		const order: string[] = [];
		const server = Bun.serve({
			port: 0,
			async fetch(request) {
				const { pathname } = new URL(request.url);
				if (pathname === '/api/v1/auth/refresh') {
					refreshCalls += 1;
					order.push('refresh-start');
					beginRefresh();
					await refreshGate;
					order.push('refresh-finish');
					return Response.json({ accessToken: 'rotated-token', expiresAt: '2030-01-01T00:00:00Z' });
				}
				if (pathname === '/api/v1/auth/logout') {
					logoutCalls += 1;
					order.push('logout');
					return Response.json({ message: 'Logged out' });
				}
				if (pathname === '/api/v1/protected') {
					return Response.json({ error: 'Unauthorized', code: 'TOKEN_INVALID' }, { status: 401 });
				}
				return Response.json({ error: 'Not found', code: 'NOT_FOUND' }, { status: 404 });
			}
		});
		const { previousWindow, previousLocalStorage } = installWindow(server.url.origin);
		clearTokens();
		setAccessToken('original-token');

		try {
			const pendingRefresh = refresh();
			await refreshStarted;
			const logout = logoutSession();
			assert.equal(logoutCalls, 0);
			await assert.rejects(refreshSession(), AuthSessionClosedError);
			releaseRefresh();
			await pendingRefresh;
			await logout;

			assert.deepEqual(order, ['refresh-start', 'refresh-finish', 'logout']);
			assert.equal(logoutCalls, 1);
			assert.equal(getAccessToken(), null);
			await assert.rejects(apiRequest('/protected'), {
				name: 'ApiRequestError',
				status: 401,
				code: 'TOKEN_INVALID'
			});
			assert.equal(refreshCalls, 1);
		} finally {
			releaseRefresh();
			clearTokens();
			Object.assign(globalThis, { window: previousWindow, localStorage: previousLocalStorage });
			server.stop(true);
		}
	});

	for (const action of ['refresh', 'logout'] as const) {
		for (const persistent of [false, true]) {
			test(`${action} retries a superseded cookie once and preserves auth on repeated conflict: ${persistent}`, async () => {
				let calls = 0;
				const server = Bun.serve({
					port: 0,
					fetch() {
						calls++;
						if (persistent || calls === 1)
							return Response.json(
								{ error: 'Rotated', code: 'REFRESH_SUPERSEDED' },
								{ status: 409 }
							);
						return Response.json({ accessToken: 'replacement', expiresAt: '2030-01-01T00:00:00Z' });
					}
				});
				const { previousWindow, previousLocalStorage } = installWindow(server.url.origin);
				setAccessToken('original-token');
				try {
					const operation = action === 'refresh' ? refreshSession() : logoutSession();
					if (persistent) {
						await assert.rejects(operation, { status: 409, code: 'REFRESH_SUPERSEDED' });
						assert.equal(getAccessToken(), 'original-token');
					} else {
						await operation;
						assert.equal(getAccessToken(), action === 'refresh' ? 'replacement' : null);
					}
					assert.equal(calls, 2);
				} finally {
					clearTokens();
					Object.assign(globalThis, { window: previousWindow, localStorage: previousLocalStorage });
					server.stop(true);
				}
			});
		}
	}

	test('logout 401 never starts an automatic refresh', async () => {
		let refreshCalls = 0;
		const server = Bun.serve({
			port: 0,
			fetch(request) {
				const { pathname } = new URL(request.url);
				if (pathname === '/api/v1/auth/logout') {
					return Response.json(
						{ error: 'Sign in required', code: 'SESSION_RECOVERY_REQUIRED' },
						{ status: 401 }
					);
				}
				if (pathname === '/api/v1/auth/refresh') refreshCalls += 1;
				return Response.json({ error: 'Not found', code: 'NOT_FOUND' }, { status: 404 });
			}
		});
		const { previousWindow, previousLocalStorage } = installWindow(server.url.origin);
		clearTokens();
		setAccessToken('original-token');

		try {
			await assert.rejects(logoutSession(), {
				name: 'ApiRequestError',
				status: 401,
				code: 'SESSION_RECOVERY_REQUIRED'
			});
			assert.equal(refreshCalls, 0);
			assert.equal(getAccessToken(), null);
		} finally {
			clearTokens();
			Object.assign(globalThis, { window: previousWindow, localStorage: previousLocalStorage });
			server.stop(true);
		}
	});
});
