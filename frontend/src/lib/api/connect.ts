import { api, apiFetch, parseResponse, ApiRequestError, getAccessToken } from './client';
import { record, array, nonEmptyString } from './parse';

export interface PairingCode {
	code: string;
	expiresAt: string;
}

export async function generatePairingCode(password: string): Promise<PairingCode> {
	let data: unknown;
	try {
		const token = getAccessToken();
		// Confirmation failures are also 401s. Never replay a password-bearing request.
		const response = await apiFetch('/connect/pairing', {
			method: 'POST',
			body: { password },
			skipAuth: true,
			headers: token ? { Authorization: `Bearer ${token}` } : {}
		});
		data = await parseResponse<unknown>(response);
	} catch (error) {
		if (error instanceof ApiRequestError && (error.status === 404 || error.status === 405)) {
			throw new Error(
				'This server does not support device pairing. Update the server, then try again.',
				{ cause: error }
			);
		}
		throw error;
	}
	if (
		typeof data !== 'object' ||
		data === null ||
		!('code' in data) ||
		typeof data.code !== 'string' ||
		!/^[23456789ABCDEFGHJKLMNPQRSTUVWXYZ]{4}-[23456789ABCDEFGHJKLMNPQRSTUVWXYZ]{4}$/.test(
			data.code
		) ||
		!('expiresAt' in data) ||
		typeof data.expiresAt !== 'string' ||
		!Number.isFinite(Date.parse(data.expiresAt))
	)
		throw new Error('The server returned an invalid pairing code. Try again.');
	return { code: data.code, expiresAt: data.expiresAt };
}

export interface PairedClient {
	clientId: string;
	clientName: string;
	username: string;
	createdAt: string;
	expiresAt: string;
}

export type PairingStatus = { enabled: false } | { enabled: true; expiresAt: string };

export async function listPairedClients(signal?: AbortSignal): Promise<PairedClient[]> {
	const data = await api.get<unknown>('/connect/clients', undefined, signal);
	const body = record(data, 'paired clients');
	return array(body.clients, 'paired clients').map((value) => {
		const client = record(value, 'paired client');
		return {
			clientId: nonEmptyString(client.clientId, 'client ID'),
			clientName: nonEmptyString(client.clientName, 'client name'),
			username: nonEmptyString(client.username, 'client username'),
			createdAt: timestamp(client.createdAt),
			expiresAt: timestamp(client.expiresAt)
		};
	});
}

export async function pairingStatus(signal?: AbortSignal): Promise<PairingStatus> {
	const data = record(
		await api.get<unknown>('/connect/pairing', undefined, signal),
		'pairing status'
	);
	if (data.enabled === false) return { enabled: false };
	if (data.enabled === true) return { enabled: true, expiresAt: timestamp(data.expiresAt) };
	throw new Error('Invalid pairing status');
}

export async function revokePairedClient(id: string): Promise<void> {
	const response = await apiFetch(`/connect/clients/${encodeURIComponent(id)}`, {
		method: 'DELETE'
	});
	if (!response.ok) await parseResponse<never>(response);
}

function timestamp(value: unknown): string {
	const text = nonEmptyString(value, 'device timestamp');
	if (!Number.isFinite(Date.parse(text))) throw new Error('Invalid device timestamp');
	return text;
}
