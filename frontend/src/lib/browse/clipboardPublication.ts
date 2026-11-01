export type BrowseClipboardPublication =
	| { kind: 'external'; revision: number }
	| { kind: 'pending'; revision: number }
	| { kind: 'published'; revision: number }
	| { kind: 'failed'; revision: number };

let nextSelectionId = 0;
let publication: BrowseClipboardPublication = { kind: 'external', revision: 0 };
let publicationQueue: Promise<void> = Promise.resolve();

/** Marks the current in-app selection as the source to use until native publication finishes. */
export function beginBrowseClipboardPublication(): number {
	const selectionId = ++nextSelectionId;
	publication = { kind: 'pending', revision: selectionId };
	return selectionId;
}

export function finishBrowseClipboardPublication(
	selectionId: number,
	result: 'published' | 'failed'
): void {
	if (publication.kind !== 'pending' || publication.revision !== selectionId) return;
	publication = { kind: result, revision: selectionId };
}

/** Serializes native writes so an older request cannot overwrite a newer selection. */
export function publishBrowseClipboardInOrder(write: () => Promise<void>): Promise<void> {
	const next = publicationQueue.catch(() => {}).then(write);
	publicationQueue = next;
	return next;
}

export function getBrowseClipboardPublication(): BrowseClipboardPublication {
	return publication;
}

export function shouldReadNativeBrowseClipboard(): boolean {
	return publication.kind === 'external' || publication.kind === 'published';
}

export function isCurrentBrowseClipboardSelection(selectionId: number): boolean {
	return publication.kind !== 'external' && publication.revision === selectionId;
}

export function clearBrowseClipboardPublication(selectionId: number): void {
	if (isCurrentBrowseClipboardSelection(selectionId)) {
		publication = { kind: 'external', revision: selectionId };
	}
}
