import { api } from './client';
import { array, boolean, nonEmptyString, optionalString, record } from './parse';

export interface PairingStatus {
	enabled: boolean;
	expiresAt: string | null;
}

export interface PairedClient {
	id: string;
	name: string;
	createdAt: string;
	expiresAt: string;
}

export async function getPairingStatus(): Promise<PairingStatus> {
	const value = record(await api.get<unknown>('/connect/pairing'), 'pairing status');
	return {
		enabled: boolean(value.enabled, 'pairing enabled'),
		expiresAt: optionalString(value.expiresAt, 'pairing expiry') ?? null
	};
}

export async function enablePairing(
	password: string
): Promise<{ code: string; expiresAt: string }> {
	const value = record(await api.post<unknown>('/connect/pairing', { password }), 'pairing code');
	return {
		code: nonEmptyString(value.code, 'pairing code'),
		expiresAt: nonEmptyString(value.expiresAt, 'pairing expiry')
	};
}

export async function disablePairing(): Promise<void> {
	await api.delete<unknown>('/connect/pairing');
}

export async function listPairedClients(): Promise<PairedClient[]> {
	const response = record(await api.get<unknown>('/connect/clients'), 'paired clients');
	return array(response.clients, 'paired clients').map((item) => {
		const value = record(item, 'paired client');
		return {
			id: nonEmptyString(value.clientId, 'client id'),
			name: nonEmptyString(value.clientName, 'client name'),
			createdAt: nonEmptyString(value.createdAt, 'client creation time'),
			expiresAt: nonEmptyString(value.expiresAt, 'client expiry')
		};
	});
}

export async function revokePairedClient(id: string): Promise<void> {
	await api.delete<unknown>(`/connect/clients/${encodeURIComponent(id)}`);
}
