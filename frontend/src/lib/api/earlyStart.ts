import { CONFIG } from '$lib/config';
import { parseResponse, type AccessTokenResponse } from './client';
import type { FileList } from './files';

/** What `static/early-fetch.js` starts while the app's JavaScript is still downloading. */
interface EarlyStart {
	/** The page-load token refresh. `null` when the server has no authentication. */
	session: Promise<unknown> | null;
	list: { url: string; startedAt: number; response: Promise<Response | null> };
}

declare global {
	interface Window {
		/** `undefined` until early-fetch.js runs; `null` once the app has taken over. */
		boxboxEarly?: EarlyStart | null;
	}
}

function isAccessTokenResponse(value: unknown): value is AccessTokenResponse {
	return (
		typeof value === 'object' &&
		value !== null &&
		'accessToken' in value &&
		typeof value.accessToken === 'string' &&
		'expiresAt' in value &&
		typeof value.expiresAt === 'string'
	);
}

/**
 * The app takes over the page-load requests the shell started, once each. The first take also marks
 * the handoff `null`, so a script that arrives late stands down instead of starting a second token
 * refresh. A take that finds nothing usable returns `null`, and the caller makes the request itself.
 */
export function createEarlyStart(win: () => Pick<Window, 'boxboxEarly'>) {
	let claimed = false;
	let session: EarlyStart['session'] = null;
	let list: EarlyStart['list'] | null = null;

	function claim(): void {
		if (claimed) return;
		claimed = true;
		const handoff = win().boxboxEarly;
		win().boxboxEarly = null;
		session = handoff?.session ?? null;
		list = handoff?.list ?? null;
	}

	return {
		async takeSession(): Promise<AccessTokenResponse | null> {
			claim();
			const pending = session;
			session = null;
			const tokens = await pending;
			return isAccessTokenResponse(tokens) ? tokens : null;
		},

		/** The first page of `url`, if it is what the shell asked for and still fresh enough to show. */
		async takeDirectory(url: string): Promise<FileList | null> {
			claim();
			const early = list;
			list = null;
			if (!early || early.url !== url) return null;
			if (Date.now() - early.startedAt > CONFIG.query.earlyListMaxAgeMs) return null;
			const response = await early.response;
			if (!response) return null;
			try {
				return await parseResponse<FileList>(response);
			} catch {
				return null;
			}
		}
	};
}

export const earlyStart = createEarlyStart(() => window);
export type EarlyStartSource = typeof earlyStart;
