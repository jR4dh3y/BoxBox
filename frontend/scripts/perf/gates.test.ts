import assert from 'node:assert/strict';
import { test } from 'node:test';
import { benchmarkRegressions, checkRatchets, browserViolations } from './gates';

const metrics = {
	listingRequests: 1,
	renderedRows: 30,
	refreshRequests: 1,
	scriptRequests: 8,
	scriptBytes: 300_000
};
const limits = {
	listingRequests: 1,
	renderedRows: 32,
	refreshRequests: 1,
	scriptRequests: 8,
	scriptBytes: 330_752
};

test('browser gate rejects duplicate listings and additional scripts', () => {
	assert.deepEqual(browserViolations(metrics, limits), []);
	assert.match(
		browserViolations({ ...metrics, listingRequests: 2 }, limits).join(),
		/listingRequests/
	);
	assert.match(
		browserViolations({ ...metrics, scriptBytes: 340_000 }, limits).join(),
		/scriptBytes/
	);
	assert.match(
		browserViolations({ ...metrics, scriptRequests: 9 }, limits).join(),
		/scriptRequests/
	);
	assert.match(
		browserViolations({ ...metrics, renderedRows: 10_000 }, limits).join(),
		/renderedRows/
	);
});

test('ratchets allow tighter budgets, but not increases or deleted limits', () => {
	assert.deepEqual(checkRatchets({ bytes: 100, requests: 2 }, { bytes: 90, requests: 2 }), []);
	assert.match(checkRatchets({ bytes: 100 }, { bytes: 101 }).join(), /bytes/);
	assert.match(checkRatchets({ requests: 2 }, {}).join(), /requests/);
	assert.throws(() => checkRatchets({ bytes: 100 }, { bytes: 'unlimited' }), /finite/);
});

test('a retired budget may go only to a successor that is no looser', () => {
	const retired = { 'list/name': 'list' };
	assert.deepEqual(checkRatchets({ 'list/name': 2300 }, { list: 2048 }, retired), []);
	assert.match(checkRatchets({ 'list/name': 2300 }, { list: 2400 }, retired).join(), /list\/name/);
	assert.match(checkRatchets({ 'list/name': 2300 }, {}, retired).join(), /list\/name/);
	assert.match(checkRatchets({ 'list/type': 2300 }, { list: 2048 }, retired).join(), /list\/type/);
});

function benchmarks(allocations: number, bytes: number, ns: number) {
	return Array.from(
		{ length: 10 },
		() => `BenchmarkListDirectory/name-2 100 ${ns} ns/op ${bytes} B/op ${allocations} allocs/op`
	).join('\n');
}

test('benchmark gate blocks material allocation/byte growth, not timing noise', () => {
	const base = benchmarks(1000, 100_000, 1000);
	assert.deepEqual(benchmarkRegressions(base, benchmarks(1000, 100_000, 5000)), []);
	assert.match(benchmarkRegressions(base, benchmarks(1100, 100_000, 1000)).join(), /allocs\/op/);
	assert.match(benchmarkRegressions(base, benchmarks(1000, 110_000, 1000)).join(), /B\/op/);
});

test('absolute benchmark ceilings prevent cumulative small regressions', () => {
	const base = benchmarks(1000, 100_000, 1000);
	const ceilings = { 'ListDirectory/name.allocations': 1000, 'ListDirectory/name.bytes': 100_000 };
	assert.deepEqual(benchmarkRegressions(base, base, ceilings), []);
	assert.match(
		benchmarkRegressions(base, benchmarks(1001, 100_000, 1000), ceilings).join(),
		/exceeds ceiling/
	);
	assert.throws(
		() => benchmarkRegressions(base, base, { 'another.bytes': 100 }),
		/Missing absolute ceiling/
	);
});

test('missing, renamed, or undersampled benchmarks cannot silently pass', () => {
	const base = benchmarks(1000, 100_000, 1000);
	assert.throws(() => benchmarkRegressions(base, ''), /Missing/);
	assert.throws(
		() =>
			benchmarkRegressions(
				base,
				'BenchmarkListDirectory/name-2 1 1000 ns/op 100000 B/op 1000 allocs/op'
			),
		/samples/
	);
	assert.throws(() => benchmarkRegressions('', ''), /No benchmarks/);
	assert.throws(() => benchmarkRegressions(base, base.replaceAll('name-2', 'type-2')), /Missing/);
});
