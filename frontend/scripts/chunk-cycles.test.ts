import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { findChunkCycles, type ManifestChunk } from './chunk-cycles';

const chunk = (file: string, imports: string[] = []): ManifestChunk => ({ file, imports });

describe('findChunkCycles', () => {
	test('finds nothing in a graph that only points one way', () => {
		const manifest = {
			app: chunk('app.js', ['vendor', 'lib']),
			lib: chunk('lib.js', ['vendor']),
			vendor: chunk('vendor.js')
		};
		assert.deepEqual(findChunkCycles(manifest), []);
	});

	test('reports a loop with the files around it', () => {
		const manifest = {
			a: chunk('a.js', ['b']),
			b: chunk('b.js', ['c']),
			c: chunk('c.js', ['a'])
		};
		assert.deepEqual(findChunkCycles(manifest), [['a.js', 'b.js', 'c.js', 'a.js']]);
	});

	test('reports a chunk that imports itself', () => {
		assert.deepEqual(findChunkCycles({ a: chunk('a.js', ['a']) }), [['a.js', 'a.js']]);
	});

	test('a diamond is not a loop', () => {
		const manifest = {
			top: chunk('top.js', ['left', 'right']),
			left: chunk('left.js', ['base']),
			right: chunk('right.js', ['base']),
			base: chunk('base.js')
		};
		assert.deepEqual(findChunkCycles(manifest), []);
	});

	test('ignores imports of chunks the manifest does not list', () => {
		assert.deepEqual(findChunkCycles({ a: chunk('a.js', ['missing']) }), []);
	});
});
