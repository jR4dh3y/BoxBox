/** What a new or changed name must be, and the names the create dialog suggests. */

/** Why `name` cannot name an entry among `siblings`, or null when it can. `name` is trimmed. */
export function nameProblem(name: string, siblings: Iterable<string>): string | null {
	if (!name) return 'Enter a name.';
	if (name === '.' || name === '..') return `“${name}” is not a valid name.`;
	if (name.includes('/')) return 'A name cannot contain “/”.';
	if (name.startsWith('\\')) return 'A name cannot start with “\\”.';
	if (name.includes('\0')) return 'A name cannot contain a null character.';
	for (const sibling of siblings) if (sibling === name) return `“${name}” already exists here.`;
	return null;
}

/** As `nameProblem`, for a new item, which the server also refuses to give a backslash. */
export function createNameProblem(name: string, siblings: Iterable<string>): string | null {
	return (
		nameProblem(name, siblings) ?? (name.includes('\\') ? 'A name cannot contain “\\”.' : null)
	);
}

/** Where the part of a name before its extension ends: the whole name for a folder or a dotfile. */
export function basenameEnd(name: string, isDir: boolean): number {
	const dot = name.lastIndexOf('.');
	return dot > 0 && !isDir ? dot : name.length;
}

/** `name`, or the first of `name 2`, `name 3`, … (before the extension) not taken. */
export function unusedName(name: string, isDir: boolean, taken: (name: string) => boolean): string {
	if (!taken(name)) return name;
	const end = basenameEnd(name, isDir);
	const stem = name.slice(0, end);
	const extension = name.slice(end);
	for (let number = 2; ; number++) {
		const candidate = `${stem} ${number}${extension}`;
		if (!taken(candidate)) return candidate;
	}
}
