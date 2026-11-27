import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { createCatalog, parseIconNodes, parseIconTags } from './emblems.js';

describe('emblem catalogue', () => {
	const nodes = parseIconNodes({
		house: [['path', { d: 'M1 1' }]],
		'gamepad-2': [
			['line', { x1: 6, x2: 10, y1: 11, y2: 11 }],
			['rect', { width: '20', height: '12' }]
		],
		bogus: [['script', { src: 'x' }]]
	});
	const catalog = createCatalog(nodes, parseIconTags({ house: ['home', 'living'] }));

	test('keeps drawable shapes and drops icons with unknown ones', () => {
		assert.deepEqual(catalog.names, ['gamepad-2', 'house']);
		assert.deepEqual(catalog.shapes('gamepad-2')?.[0], {
			tag: 'line',
			attrs: { x1: '6', x2: '10', y1: '11', y2: '11' }
		});
		assert.equal(catalog.shapes('bogus'), null);
		assert.equal(catalog.shapes('constructor'), null);
	});

	test('searches names and tags by every word', () => {
		assert.deepEqual(catalog.search('home'), ['house']);
		assert.deepEqual(catalog.search('Game pad'), ['gamepad-2']);
		assert.deepEqual(catalog.search('home pad'), []);
	});
});
