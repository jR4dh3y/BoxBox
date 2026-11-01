/** Address-bar paths use the same root/home mounts as the desktop launcher. */
export function desktopInputPath(value: string): string {
	let path = value.trim();
	if (path.startsWith('file:')) {
		const url = new URL(path);
		if (
			url.protocol !== 'file:' ||
			(url.hostname && url.hostname !== 'localhost') ||
			url.search ||
			url.hash
		)
			throw new Error('Enter a local file path');
		path = decodeURIComponent(url.pathname);
	}
	if (path === '~') return 'home';
	if (path.startsWith('~/')) return `home/${cleanSegments(path.slice(2))}`.replace(/\/$/, '');
	if (path.startsWith('/')) return `root/${cleanSegments(path)}`.replace(/\/$/, '');
	return path;
}

function cleanSegments(path: string): string {
	const segments: string[] = [];
	for (const part of path.split('/')) {
		if (!part || part === '.') continue;
		if (part === '..') segments.pop();
		else segments.push(part);
	}
	return segments.join('/');
}
