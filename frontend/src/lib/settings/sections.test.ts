import assert from 'node:assert/strict';
import { test } from 'node:test';
import { initialSettingsSection, settingsSections } from './sections';

test('local desktop settings replace Account with Connections; the web UI keeps Account', () => {
	const desktop = settingsSections(true).map((section) => section.id);
	const web = settingsSections(false).map((section) => section.id);
	assert.deepEqual(desktop, ['general', 'appearance', 'links', 'connections']);
	assert.deepEqual(web, ['general', 'appearance', 'links', 'account']);
});

test('a requested section that is not shown opens General instead of a hidden panel', () => {
	const desktop = settingsSections(true);
	const web = settingsSections(false);
	assert.equal(initialSettingsSection(desktop, 'connections'), 'connections');
	assert.equal(initialSettingsSection(desktop, 'account'), 'general');
	assert.equal(initialSettingsSection(web, 'account'), 'account');
	assert.equal(initialSettingsSection(web, 'connections'), 'general');
	assert.equal(initialSettingsSection(web, 'appearance'), 'appearance');
});
