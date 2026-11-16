import { fetchLoginMethods, type LoginMethods } from '#lib/api/authConfig.js';
import { ApiRequestError } from '#lib/api/client.js';

type LoginMethodsState =
	{ kind: 'loading' } | { kind: 'ready'; methods: LoginMethods } | { kind: 'error' };

/** Which sign-in controls the login page shows. The server enforces the same choice. */
export class LoginMethodsStore {
	state = $state<LoginMethodsState>({ kind: 'loading' });

	async load(fetch: () => Promise<LoginMethods> = fetchLoginMethods): Promise<void> {
		this.state = { kind: 'loading' };
		try {
			this.state = { kind: 'ready', methods: await fetch() };
		} catch (error) {
			// Older servers answer a missing route with 404 or the HTML app shell.
			this.state =
				error instanceof ApiRequestError &&
				(error.status === 404 || (error.status === 200 && error.code === 'INVALID_RESPONSE'))
					? { kind: 'ready', methods: { kind: 'password' } }
					: { kind: 'error' };
		}
	}
}
