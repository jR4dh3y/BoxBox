/**
 * Scripts the page shell loads by URL, such as `/early-fetch.js`. The Vite manifest lists only the hashed
 * app chunks, so a script that is not a chunk would otherwise load on every page without being measured.
 * Returns paths relative to the build directory.
 */
export function shellScriptPaths(html: string): string[] {
	const paths: string[] = [];
	for (const [, src] of html.matchAll(/<script\b[^>]*\ssrc="([^"]+)"/g)) {
		if (src.startsWith('/') && !src.startsWith('//') && !src.startsWith('/_app/')) {
			paths.push(src.slice(1).split(/[?#]/)[0]);
		}
	}
	return paths;
}
