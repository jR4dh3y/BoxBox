/**
 * A grid tile's name in two centred lines at most, as Dolphin and the desktop app draw it:
 * the first line ends at a word break that fits, and a second line too long to fit loses
 * its middle, so the end of the name (`(1).pdf`) stays readable.
 */

/** Where each character starts in `text`, as `[index, x]`, and the text's width. */
export type Measure = (text: string) => { starts: [number, number][]; width: number };

const ELLIPSIS = '…';

/** The first line, and the rest when it does not fit on one line. */
export function tileLabelLines(
	name: string,
	width: number,
	measure: Measure
): [string, string | null] {
	const { starts, width: total } = measure(name);
	if (total <= width) return [name, null];
	// The first character that does not fit whole, or the second, so a line is never empty.
	let overflow = name.length;
	for (let index = 0; index < starts.length; index++) {
		const end = starts[index + 1]?.[1] ?? total;
		if (end > width) {
			overflow = starts[index]?.[0] ?? name.length;
			break;
		}
	}
	overflow = Math.max(overflow, starts[1]?.[0] ?? name.length);
	const firstEnd = wordBreak(name, overflow) ?? overflow;
	const first = name.slice(0, firstEnd).trimEnd();
	const rest = name.slice(firstEnd).trimStart();
	return [first, rest ? fitMiddle(rest, width, measure) : null];
}

/** The last break before `end` that leaves a word on the line: after a space, `-`, `_` or `.`. */
function wordBreak(name: string, end: number): number | null {
	const line = name.slice(0, end);
	const breaks = [' ', '-', '_', '.']
		.map((separator) => line.lastIndexOf(separator))
		.filter((index) => index >= 0)
		.map((index) => index + 1);
	const best = Math.max(0, ...breaks);
	return best > 0 && best <= end ? best : null;
}

/** `text` whole if it fits, else its start and end around "…". */
function fitMiddle(text: string, width: number, measure: Measure): string {
	const { starts, width: total } = measure(text);
	if (total <= width) return text;
	const half = Math.max(width - measure(ELLIPSIS).width, 0) / 2;
	// The start keeps characters that end by `half`; the end keeps those that begin `half`
	// before the end.
	let head = 0;
	for (let index = 0; index < starts.length; index++) {
		const end = starts[index + 1] ?? [text.length, total];
		if (end[1] > half) break;
		head = end[0];
	}
	const tail = starts.find(([, x]) => total - x <= half)?.[0] ?? text.length;
	return `${text.slice(0, head).trimEnd()}${ELLIPSIS}${text.slice(Math.max(tail, head)).trimStart()}`;
}
