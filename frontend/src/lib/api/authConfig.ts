import { apiRequest } from './client';
import { boolean, nonEmptyString, record } from './parse';

/** The sign-in methods the server accepts. */
export type LoginMethods =
	| { kind: 'password' }
	| { kind: 'password-and-sso'; providerName: string }
	| { kind: 'sso'; providerName: string };

/** A full-page navigation target; the server redirects on to the identity provider. */
export const SSO_LOGIN_PATH = '/api/v1/auth/oidc/login';

export function parseLoginMethods(value: unknown): LoginMethods {
	const config = record(value, 'auth config');
	const passwordLogin = boolean(config.passwordLogin, 'auth config passwordLogin');
	if (config.oidc === null) {
		if (!passwordLogin) throw new Error('Invalid auth config: no sign-in method');
		return { kind: 'password' };
	}
	const providerName = nonEmptyString(
		record(config.oidc, 'auth config oidc').name,
		'auth config oidc name'
	);
	return passwordLogin ? { kind: 'password-and-sso', providerName } : { kind: 'sso', providerName };
}

export async function fetchLoginMethods(): Promise<LoginMethods> {
	return parseLoginMethods(await apiRequest('/auth/config', { skipAuth: true }));
}
