/** Codes the server puts in `/login?sso_error=` after a failed single sign-on. */
const messages: Record<string, string> = {
	provider_unavailable: 'The sign-in provider is unavailable. Try again later.',
	flow_expired: 'Sign-in expired or was started elsewhere. Try again.',
	denied: 'Sign-in was cancelled.',
	provider_error: 'The sign-in provider rejected the request.',
	identity_invalid: 'The sign-in provider did not send a usable username.',
	not_allowed: "This account isn't allowed to use BoxBox.",
	server_error: 'Sign-in failed. Try again.'
};

/** The message for a code. Unknown codes get a generic message, so query text is never shown. */
export function ssoErrorMessage(code: string | null): string | null {
	if (code === null) return null;
	return Object.hasOwn(messages, code) ? messages[code] : messages.server_error;
}
