import { fetchLoginMethods, type LoginMethods } from '#lib/api/authConfig.js';

type LoginMethodsState = { kind: 'loading' } | { kind: 'ready'; methods: LoginMethods };

/** Which sign-in controls the login page shows. The server enforces the same choice. */
export class LoginMethodsStore {
	state = $state<LoginMethodsState>({ kind: 'loading' });

	async load(fetch: () => Promise<LoginMethods> = fetchLoginMethods): Promise<void> {
		try {
			this.state = { kind: 'ready', methods: await fetch() };
		} catch {
			// Older servers have no config route; the password form is what they accept.
			this.state = { kind: 'ready', methods: { kind: 'password' } };
		}
	}
}
