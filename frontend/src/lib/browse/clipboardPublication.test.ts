import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
	beginBrowseClipboardPublication,
	clearBrowseClipboardPublication,
	finishBrowseClipboardPublication,
	getBrowseClipboardPublication,
	isCurrentBrowseClipboardSelection,
	publishBrowseClipboardInOrder,
	shouldReadNativeBrowseClipboard
} from './clipboardPublication';

test('failed native publication keeps paste on the matching in-app selection', () => {
	const selectionId = beginBrowseClipboardPublication();

	assert.equal(shouldReadNativeBrowseClipboard(), false);
	finishBrowseClipboardPublication(selectionId, 'failed');
	assert.deepEqual(getBrowseClipboardPublication(), { kind: 'failed', revision: selectionId });
	assert.equal(shouldReadNativeBrowseClipboard(), false);
});

test('stale publication results cannot replace a newer clipboard selection', () => {
	const oldSelectionId = beginBrowseClipboardPublication();
	const currentSelectionId = beginBrowseClipboardPublication();

	finishBrowseClipboardPublication(oldSelectionId, 'published');
	assert.deepEqual(getBrowseClipboardPublication(), {
		kind: 'pending',
		revision: currentSelectionId
	});
	assert.equal(shouldReadNativeBrowseClipboard(), false);

	finishBrowseClipboardPublication(currentSelectionId, 'published');
	assert.equal(shouldReadNativeBrowseClipboard(), true);
	assert.equal(isCurrentBrowseClipboardSelection(currentSelectionId), true);

	clearBrowseClipboardPublication(currentSelectionId);
	assert.deepEqual(getBrowseClipboardPublication(), {
		kind: 'external',
		revision: currentSelectionId
	});
	assert.equal(shouldReadNativeBrowseClipboard(), true);
});

test('native clipboard writes run in selection order', async () => {
	const calls: string[] = [];
	let finishFirst = () => {};
	const first = publishBrowseClipboardInOrder(async () => {
		calls.push('first');
		await new Promise<void>((resolve) => {
			finishFirst = resolve;
		});
	});
	const second = publishBrowseClipboardInOrder(async () => {
		calls.push('second');
	});

	await new Promise<void>((resolve) => setTimeout(resolve, 0));
	assert.deepEqual(calls, ['first']);
	finishFirst();
	await Promise.all([first, second]);
	assert.deepEqual(calls, ['first', 'second']);
});
