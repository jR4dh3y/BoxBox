import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
	TypeAhead,
	cursorDelta,
	cursorStep,
	findPrefix,
	moveCursor,
	toggleAt,
	typedCharacter
} from './cursor.js';

const paths = ['a', 'b', 'c', 'd', 'e'];
const none = { paths: new Set<string>(), anchor: null };

describe('keyboard cursor', () => {
	test('keys move by rows, columns, pages and ends', () => {
		const grid = { grid: true, columns: 3, page: 6 };
		assert.equal(cursorDelta('ArrowDown', grid), 3);
		assert.equal(
			cursorDelta('ArrowLeft', { grid: false, columns: 1, page: 6 }),
			null,
			'a list has no Left'
		);
		assert.equal(
			cursorDelta('ArrowLeft', { grid: true, columns: 1, page: 6 }),
			-1,
			'a narrow grid has'
		);
		assert.equal(cursorDelta('ArrowLeft', grid), -1);
		assert.equal(cursorStep(2, 5, cursorDelta('End', grid) ?? 0), 4);
		assert.equal(cursorDelta('PageUp', grid), -6);
		assert.equal(cursorDelta('x', grid), null);
	});

	test('the first key starts at the top, or at the end going up', () => {
		assert.equal(cursorStep(null, 5, 1), 0);
		assert.equal(cursorStep(null, 5, -1), 4);
		assert.equal(cursorStep(2, 5, 10), 4);
		assert.equal(cursorStep(2, 5, -10), 0);
		assert.equal(cursorStep(null, 0, 1), null);
	});

	test('Home and End reach their ends without a cursor', () => {
		const list = { grid: false, columns: 1, page: 6 };
		assert.equal(cursorStep(null, 5, cursorDelta('Home', list) ?? 0), 0);
		assert.equal(cursorStep(null, 5, cursorDelta('End', list) ?? 0), 4);
	});

	test('a plain move selects only the item; Shift extends from the anchor; Ctrl keeps it', () => {
		const selected = moveCursor(paths, none, null, 1, 'select');
		assert.deepEqual([...selected.paths], ['b']);
		const extended = moveCursor(paths, selected, 1, 3, 'extend');
		assert.deepEqual([...extended.paths], ['b', 'c', 'd']);
		const back = moveCursor(paths, extended, 3, 0, 'extend');
		assert.deepEqual([...back.paths], ['a', 'b']);
		assert.equal(moveCursor(paths, back, 0, 4, 'focus'), back);
	});

	test('Ctrl+Space toggles the item and makes it the anchor', () => {
		const one = toggleAt(paths, none, 2);
		assert.deepEqual([...one.paths], ['c']);
		assert.equal(one.anchor, 2);
		assert.deepEqual([...toggleAt(paths, one, 2).paths], []);
	});

	test('typing jumps to a name, and a repeated letter steps through matches', () => {
		const names = ['apple', 'Banana', 'berry', 'cherry'];
		assert.equal(findPrefix(names, 'BER', 0), 2);
		const typing = new TypeAhead();
		assert.equal(typing.type('b', 0, names, null), 1);
		assert.equal(typing.type('e', 100, names, 1), 2);
		const repeating = new TypeAhead();
		assert.equal(repeating.type('b', 0, names, null), 1);
		assert.equal(repeating.type('b', 100, names, 1), 2);
		assert.equal(repeating.type('b', 200, names, 2), 1, 'wraps round');
		assert.equal(repeating.type('c', 5000, names, 1), 3, 'a pause starts a new name');
	});

	test('only printable keys without modifiers type', () => {
		const key = (k: string, ctrl = false) => ({
			key: k,
			ctrlKey: ctrl,
			altKey: false,
			metaKey: false
		});
		assert.equal(typedCharacter(key('a')), 'a');
		assert.equal(typedCharacter(key('a', true)), null);
		assert.equal(typedCharacter(key('ArrowUp')), null);
		assert.equal(typedCharacter(key(' ')), null);
	});
});
