import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { basenameEnd, createNameProblem, nameProblem, unusedName } from './names.js';

describe('names', () => {
	test('names are refused with a reason', () => {
		assert.equal(nameProblem('', []), 'Enter a name.');
		assert.equal(nameProblem('a/b', []), 'A name cannot contain “/”.');
		assert.equal(nameProblem('..', []), '“..” is not a valid name.');
		assert.equal(nameProblem('a.txt', ['a.txt']), '“a.txt” already exists here.');
		// A backslash is an ordinary character on Linux.
		assert.equal(nameProblem('a\\b', []), null);
		// The server's create refuses one, though.
		assert.equal(createNameProblem('a\\b', []), 'A name cannot contain “\\”.');
	});

	test('suggestions skip taken names before the extension', () => {
		const taken = (name: string) => ['untitled.txt', 'untitled 2.txt', 'New folder'].includes(name);
		assert.equal(unusedName('untitled.txt', false, taken), 'untitled 3.txt');
		assert.equal(unusedName('New folder', true, taken), 'New folder 2');
		assert.equal(unusedName('notes', false, taken), 'notes');
		assert.equal(basenameEnd('.bashrc', false), 7);
		assert.equal(basenameEnd('my.folder', true), 9);
	});
});
