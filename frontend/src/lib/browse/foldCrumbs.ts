/** Long paths fold their middle crumbs into a menu, keeping the first and the last three. */
export function foldCrumbs<T>(crumbs: readonly T[], keep = 4): { shown: T[]; folded: T[] } {
	if (crumbs.length <= keep) return { shown: [...crumbs], folded: [] };
	const [first, ...rest] = crumbs;
	const tail = rest.slice(-(keep - 1));
	const folded = rest.slice(0, rest.length - tail.length);
	return { shown: first === undefined ? tail : [first, ...tail], folded };
}
