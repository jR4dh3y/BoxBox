import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { HistoryTrail, type TrailStorage } from './historyTrail.js';

function memory(): TrailStorage & { data: Map<string, string> } {
	const data = new Map<string, string>();
	return {
		data,
		getItem: (key) => data.get(key) ?? null,
		setItem: (key, value) => void data.set(key, value)
	};
}

describe('history trail', () => {
	test('remembers each entry’s folder and the selection left there', () => {
		const trail = new HistoryTrail(memory());
		trail.arrive(0, '', true);
		trail.arrive(1, 'home', true);
		trail.leave(1, ['home/a', 'home/b']);
		trail.arrive(2, 'home/a', true);
		assert.deepEqual(trail.at(1), { path: 'home', selected: ['home/a', 'home/b'] });
		// Coming back to an entry keeps what was remembered there.
		trail.arrive(1, 'home', false);
		assert.deepEqual(trail.at(1)?.selected, ['home/a', 'home/b']);
	});

	test('a new entry forgets what was forward of it', () => {
		const trail = new HistoryTrail(memory());
		trail.arrive(0, '', true);
		trail.arrive(1, 'a', true);
		trail.arrive(2, 'b', true);
		trail.arrive(1, 'a', false);
		trail.arrive(2, 'c', true);
		assert.deepEqual(
			trail.around(2, true, 2).map(({ entry }) => entry.path),
			['a', '']
		);
		assert.deepEqual(
			trail.around(0, false, 2).map(({ index }) => index),
			[1, 2]
		);
	});

	test('a reload keeps the trail, and unreadable storage starts empty', () => {
		const storage = memory();
		new HistoryTrail(storage).arrive(0, 'home', true);
		assert.equal(new HistoryTrail(storage).at(0)?.path, 'home');
		storage.data.set('boxbox_history_trail', '{oops');
		assert.equal(new HistoryTrail(storage).at(0), null);
	});
});
