import assert from 'node:assert/strict';
import { test } from 'node:test';
import { addMenuItems, toAddAction } from './addMenu';

test('creating and uploading are disabled separately', () => {
	const readOnlyForUploads = addMenuItems(true, false);
	const disabled = (id: string) => readOnlyForUploads.find((item) => item.id === id)?.disabled;
	assert.equal(disabled('new-folder'), false);
	assert.equal(disabled('upload-files'), true);
});

test('only menu actions are accepted', () => {
	assert.equal(toAddAction('upload-folder'), 'upload-folder');
	assert.equal(toAddAction('separator'), undefined);
});
