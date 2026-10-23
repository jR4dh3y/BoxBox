// Starts the folder listing while the app's JavaScript downloads, so it is already on its way when
// the app asks for it. src/lib/api/earlyStart.ts takes over what this starts.
// The request must match listDirectory in src/lib/api/files.ts; earlyStart.test.ts checks that.
(() => {
	// `null` means the app started first and took over. Start nothing then: a second token refresh
	// would race the app's own, and each refresh rotates the cookie.
	if (window.boxboxEarly !== undefined) return;

	const { pathname, searchParams } = new URL(location.href);
	const path = (searchParams.get('path') ?? '').replace(/^\/+|\/+$/g, '');
	if (pathname !== '/browse' || !path) return;

	const url = new URL(
		`/api/v1/files/list/${path.split('/').map(encodeURIComponent).join('/')}`,
		location.origin
	);

	const list = (token) =>
		fetch(url, {
			headers: token ? { Authorization: `Bearer ${token}` } : {},
			credentials: 'same-origin'
		}).then(
			(response) => (response.ok ? response : null),
			() => null
		);
	const isDevelopment =
		document.querySelector('meta[name="boxbox-dev-mode"]')?.getAttribute('content') === 'true';
	const session = isDevelopment
		? null
		: fetch('/api/v1/auth/refresh', { method: 'POST', credentials: 'same-origin' })
				.then((response) =>
					response.ok
						? response.json()
						: response.status === 401
							? response.json().then(
									(body) => ({ rejected: true, error: body?.error, code: body?.code }),
									() => ({ rejected: true })
								)
							: null
				)
				.catch(() => null);

	window.boxboxEarly = {
		session,
		list: {
			url: url.href,
			startedAt: Date.now(),
			response: session
				? session.then((tokens) => (typeof tokens?.accessToken === 'string' ? list(tokens.accessToken) : null))
				: list()
		}
	};
})();
