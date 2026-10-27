interface ScrollAnchor {
	itemKey: string | null;
	itemIndex: number;
	offset: number;
}

const MAX_STORED_POSITIONS = 32;
const positions = new Map<string, ScrollAnchor>();

/** Scroll anchors survive view teardown without becoming reactive application state. */
export function rememberPosition(
	key: string,
	itemKey: string | null,
	itemIndex: number,
	offset: number
) {
	const existing = positions.get(key);
	if (existing) {
		existing.itemKey = itemKey;
		existing.itemIndex = itemIndex;
		existing.offset = offset;
		positions.delete(key);
		positions.set(key, existing);
	} else {
		positions.set(key, { itemKey, itemIndex, offset });
	}
	if (positions.size > MAX_STORED_POSITIONS) {
		const oldest = positions.keys().next();
		if (!oldest.done) positions.delete(oldest.value);
	}
}

export function recallPosition(key: string): ScrollAnchor | null {
	const position = positions.get(key);
	if (!position) return null;
	positions.delete(key);
	positions.set(key, position);
	return position;
}

export type { ScrollAnchor };
