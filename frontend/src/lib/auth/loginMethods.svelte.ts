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
			// Older servers have no config route; the password form is what they accept.
			this.state =
				error instanceof ApiRequestError && error.status === 404
					? { kind: 'ready', methods: { kind: 'password' } }
					: { kind: 'error' };
		}
	}
}
