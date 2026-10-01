/**
 * WebSocket messages sent by the server, parsed from raw frames
 */

import { isJobState, type JobUpdate } from './jobs';

export type WSServerMessage =
	| { type: 'job_update' | 'job_complete'; payload: JobUpdate }
	| { type: 'error'; payload: { message: string } }
	| { type: 'pong' };

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null;
}

function parseJobUpdate(payload: unknown): JobUpdate | null {
	if (!isRecord(payload)) return null;
	const { jobId, state, progress, error } = payload;
	if (typeof jobId !== 'string' || !isJobState(state) || typeof progress !== 'number') {
		return null;
	}
	return { jobId, state, progress, error: typeof error === 'string' ? error : undefined };
}

/** Returns null for malformed JSON, an unknown type, or a payload of the wrong shape. */
export function parseServerMessage(text: string): WSServerMessage | null {
	let value: unknown;
	try {
		value = JSON.parse(text);
	} catch {
		return null;
	}
	if (!isRecord(value)) return null;

	const { type, payload } = value;
	switch (type) {
		case 'job_update':
		case 'job_complete': {
			const update = parseJobUpdate(payload);
			return update ? { type, payload: update } : null;
		}
		case 'error':
			return isRecord(payload) && typeof payload.message === 'string'
				? { type, payload: { message: payload.message } }
				: null;
		case 'pong':
			return { type };
		default:
			return null;
	}
}
