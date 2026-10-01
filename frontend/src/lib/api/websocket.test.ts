import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { parseServerMessage } from './websocket';

const jobUpdate = (payload: object) => JSON.stringify({ type: 'job_update', payload });

describe('parseServerMessage', () => {
	test('accepts job updates and completions', () => {
		assert.deepEqual(
			parseServerMessage(jobUpdate({ jobId: 'a', state: 'running', progress: 40 })),
			{
				type: 'job_update',
				payload: { jobId: 'a', state: 'running', progress: 40, error: undefined }
			}
		);
		assert.deepEqual(
			parseServerMessage(
				JSON.stringify({
					type: 'job_complete',
					payload: { jobId: 'a', state: 'failed', progress: 10, error: 'boom' }
				})
			),
			{
				type: 'job_complete',
				payload: { jobId: 'a', state: 'failed', progress: 10, error: 'boom' }
			}
		);
	});

	test('accepts error and pong messages', () => {
		assert.deepEqual(parseServerMessage('{"type":"error","payload":{"message":"nope"}}'), {
			type: 'error',
			payload: { message: 'nope' }
		});
		assert.deepEqual(parseServerMessage('{"type":"pong"}'), { type: 'pong' });
	});

	test('rejects a job update with a wrong payload', () => {
		for (const payload of [
			{ jobId: 'a', state: 'exploded', progress: 1 },
			{ jobId: 'a', state: 'running', progress: '40' },
			{ jobId: 'a', state: 'running', progress: -1 },
			{ jobId: 'a', state: 'running', progress: 101 },
			{ jobId: 'a', state: 'running', progress: 12.5 },
			{ state: 'running', progress: 1 }
		]) {
			assert.equal(parseServerMessage(jobUpdate(payload)), null, JSON.stringify(payload));
		}
		assert.equal(parseServerMessage('{"type":"job_update"}'), null);
		assert.equal(parseServerMessage('{"type":"error","payload":{}}'), null);
	});

	test('rejects unknown types and input that is not a message', () => {
		for (const text of ['{"type":"hello"}', 'not json', '[1,2]', 'null', '"pong"']) {
			assert.equal(parseServerMessage(text), null, text);
		}
	});
});
