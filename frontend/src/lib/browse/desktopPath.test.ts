import { test } from 'node:test';
import assert from 'node:assert/strict';
import { desktopInputPath } from './desktopPath';

test('desktop addresses accept OS paths while preserving virtual paths', () => {
	assert.equal(desktopInputPath('/'), 'root');
	assert.equal(desktopInputPath('/etc/../usr/share'), 'root/usr/share');
	assert.equal(desktopInputPath('~/Downloads'), 'home/Downloads');
	assert.equal(desktopInputPath('~'), 'home');
	assert.equal(desktopInputPath('home/Downloads'), 'home/Downloads');
	assert.equal(desktopInputPath('file:///tmp/a%20file'), 'root/tmp/a file');
	assert.throws(() => desktopInputPath('file://another-host/share'));
});
