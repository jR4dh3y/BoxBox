/**
 * Select by Pattern (Ctrl+S): a pattern with `*` or `?` must match the whole name; plain text
 * matches anywhere in it. Case is ignored either way, as in the desktop app.
 */
export function matchesPattern(pattern: string, name: string): boolean {
	const wanted = pattern.trim().toLocaleLowerCase();
	const lower = name.toLocaleLowerCase();
	if (/[*?]/.test(wanted)) return glob(Array.from(wanted), Array.from(lower));
	return wanted !== '' && lower.includes(wanted);
}

/** `*` matches any run of characters and `?` exactly one. */
function glob(pattern: readonly string[], name: readonly string[]): boolean {
	let p = 0;
	let n = 0;
	// Where the last `*` was, and how much of the name it has swallowed so far.
	let star: { p: number; n: number } | null = null;
	while (n < name.length) {
		if (p < pattern.length && (pattern[p] === '?' || pattern[p] === name[n])) {
			p++;
			n++;
		} else if (p < pattern.length && pattern[p] === '*') {
			star = { p, n };
			p++;
		} else if (star) {
			star = { p: star.p, n: star.n + 1 };
			p = star.p + 1;
			n = star.n;
		} else return false;
	}
	while (pattern[p] === '*') p++;
	return p === pattern.length;
}

/** Every item whose name the pattern matches. */
export function pathsMatching(
	items: readonly { name: string; path: string }[],
	pattern: string
): Set<string> {
	return new Set(
		items.filter((item) => matchesPattern(pattern, item.name)).map((item) => item.path)
	);
}

/** Ctrl+I: every item not picked, and none of those that were. */
export function invertedPaths(
	items: readonly { path: string }[],
	selected: ReadonlySet<string>
): Set<string> {
	return new Set(items.filter((item) => !selected.has(item.path)).map((item) => item.path));
}
