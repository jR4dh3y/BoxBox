// Starts the first folder listing while the app's JavaScript downloads, so the page of files is already
// on its way when the app asks for it. src/lib/api/earlyStart.ts takes over what this starts.
// The request must match listDirectory in src/lib/api/files.ts; earlyStart.test.ts checks that.
(() => {
	// `null` means the app started first and took over. Start nothing then: a second token refresh
	// would race the app's own, and each refresh rotates the cookie.
	if (window.boxboxEarly !== undefined) return;

	const { pathname, searchParams } = new URL(location.href);
	const path = (searchParams.get('path') ?? '').replace(/^\/+|\/+$/g, '');
	if (pathname !== '/browse' || !path) return;

	let settings = {};
	try {
		settings = JSON.parse(localStorage.getItem('boxbox_settings')) ?? {};
	} catch {
		// Unreadable settings: use the defaults, as the app does.
	}
	const sort = searchParams.get('sort');
	const dir = searchParams.get('dir');
	const url = new URL(
		`/api/v1/files/list/${path.split('/').map(encodeURIComponent).join('/')}`,
		location.origin
	);
	url.searchParams.append('page', 1);
	url.searchParams.append('pageSize', 50);
	url.searchParams.append(
		'sortBy',
		['name', 'size', 'modTime', 'type'].includes(sort) ? sort : (settings.defaultSortBy ?? 'name')
	);
	url.searchParams.append(
		'sortDir',
		dir === 'asc' || dir === 'desc' ? dir : (settings.defaultSortDir ?? 'asc')
	);
	url.searchParams.append('includeHidden', settings.showHiddenFiles ?? false);

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
				.then((response) => (response.ok ? response.json() : null))
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
