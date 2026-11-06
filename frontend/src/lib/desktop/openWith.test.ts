import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
	filterOpenWithChoices,
	initialOpenWithChoice,
	visibleOpenWithChoice,
	withDefaultApplication
} from './openWith';

const app = (id: string, name: string, isDefault = false) => ({ id, name, isDefault });
const apps = {
	mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
	recommended: [
		app('chatgpt.desktop', 'ChatGPT'),
		app('impress.desktop', 'LibreOffice Impress', true)
	],
	other: [app('org.gnome.Nautilus.desktop', 'Files'), app('xarchiver.desktop', 'Xarchiver')]
};

test('the default handler is chosen first, and nothing is preselected without recommendations', () => {
	assert.equal(initialOpenWithChoice(apps), 'impress.desktop');
	assert.equal(
		initialOpenWithChoice({ ...apps, recommended: [app('chatgpt.desktop', 'ChatGPT')] }),
		'chatgpt.desktop'
	);
	assert.equal(initialOpenWithChoice({ ...apps, recommended: [] }), '');
});

test('search spans recommended and other apps, and a hidden choice cannot be opened', () => {
	const all = filterOpenWithChoices(apps, '  ');
	assert.equal(all.recommended.length + all.other.length, 4);
	const found = filterOpenWithChoices(apps, 'ARCH');
	assert.deepEqual(
		[...found.recommended, ...found.other].map((entry) => entry.id),
		['xarchiver.desktop']
	);
	assert.equal(visibleOpenWithChoice(found, 'impress.desktop'), '');
	assert.equal(visibleOpenWithChoice(found, 'xarchiver.desktop'), 'xarchiver.desktop');
	assert.deepEqual(filterOpenWithChoices(apps, 'nautilus').other, [apps.other[0]]);
});

test('a confirmed default moves an other app into recommendations without mutating the loaded list', () => {
	const updated = withDefaultApplication(apps, 'xarchiver.desktop');
	assert.equal(updated.recommended[0].id, 'xarchiver.desktop');
	assert.equal(updated.recommended[0].isDefault, true);
	assert.equal(updated.recommended.filter((app) => app.isDefault).length, 1);
	assert.ok(!updated.other.some((app) => app.id === 'xarchiver.desktop'));
	assert.equal(initialOpenWithChoice(updated), 'xarchiver.desktop');
	assert.equal(initialOpenWithChoice(apps), 'impress.desktop');
});
