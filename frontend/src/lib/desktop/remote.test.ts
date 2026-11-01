import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { PendingAction, RemoteValue } from './remote.svelte';

function deferred<T>() {
	let resolve: (value: T) => void = () => {
		throw new Error('request did not start');
	};
	const promise = new Promise<T>((done) => (resolve = done));
	return { promise, resolve };
}

describe('remote value', () => {
	test('an initial failure becomes a retryable failed state', async () => {
		const remote = new RemoteValue<string>(async () => {
			throw new Error('gio not available');
		}, 'fallback');
		await remote.load();
		assert.deepEqual(remote.state, { kind: 'failed', message: 'gio not available' });
		assert.equal(remote.refreshing, false);
	});

	test('a failed refresh keeps the last value and reports beside it', async () => {
		let calls = 0;
		const remote = new RemoteValue(async () => {
			calls += 1;
			if (calls === 1) return 'first';
			throw new Error('The service is unavailable');
		}, 'fallback');
		await remote.load();
		await remote.load();
		assert.equal(remote.value, 'first');
		assert.equal(remote.refreshError, 'The service is unavailable');
		assert.equal(remote.refreshing, false);
	});

	test('uses the fallback message for errors without one', async () => {
		const remote = new RemoteValue<string>(async () => {
			throw new Error('');
		}, 'Trash could not be loaded');
		await remote.load();
		assert.deepEqual(remote.state, { kind: 'failed', message: 'Trash could not be loaded' });
	});

	test('ignores an older response that arrives last', async () => {
		const requests = [deferred<string>(), deferred<string>()];
		let next = 0;
		const remote = new RemoteValue(() => requests[next++].promise, 'fallback');
		const first = remote.load();
		const second = remote.load();
		requests[1].resolve('newest');
		await second;
		requests[0].resolve('stale');
		await first;
		assert.equal(remote.value, 'newest');
	});

	test('an action snapshot outranks a load that was already in flight', async () => {
		const request = deferred<string>();
		const remote = new RemoteValue(() => request.promise, 'fallback');
		const load = remote.load();
		remote.set('from action');
		assert.equal(remote.refreshing, false);
		request.resolve('stale');
		await load;
		assert.equal(remote.value, 'from action');
	});

	test('an aborted load leaves the state alone', async () => {
		const remote = new RemoteValue<string>(async () => {
			throw new DOMException('aborted', 'AbortError');
		}, 'fallback');
		await remote.load();
		assert.deepEqual(remote.state, { kind: 'loading' });
		assert.equal(remote.refreshing, false);
	});
});

describe('pending action', () => {
	test('runs one action at a time', async () => {
		const action = new PendingAction<'mount'>('fallback');
		const operation = deferred<void>();
		const first = action.run('a', 'mount', () => operation.promise);
		assert.equal(action.isRunning('a', 'mount'), true);
		assert.equal(await action.run('b', 'mount', async () => {}), false);
		operation.resolve();
		assert.equal(await first, true);
		assert.equal(action.busy, false);
	});

	test('keeps the failure until reset', async () => {
		const action = new PendingAction<'eject'>('fallback');
		const done = await action.run('a', 'eject', async () => {
			throw new Error('target is busy');
		});
		assert.equal(done, false);
		assert.equal(action.error, 'target is busy');
		action.reset();
		assert.equal(action.error, null);
	});
});
