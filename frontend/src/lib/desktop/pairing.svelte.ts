import {
	disablePairing,
	enablePairing,
	getPairingStatus,
	listPairedClients,
	revokePairedClient
} from '#lib/api/connect.js';
import { PendingAction, RemoteValue } from './remote.svelte';

/** Server-side consent. Codes and passwords live only in this settings session. */
export class PairingController {
	readonly status = new RemoteValue(getPairingStatus, 'Connect mode could not be loaded');
	readonly clients = new RemoteValue(listPairedClients, 'Paired devices could not be loaded');
	readonly action = new PendingAction<'enable' | 'disable' | 'revoke'>(
		'Connect mode could not be changed'
	);
	password = $state('');
	grant = $state<{ code: string; expiresAt: string } | null>(null);

	load(): Promise<void[]> {
		return Promise.all([this.status.load(), this.clients.load()]);
	}

	async enable(): Promise<void> {
		const password = this.password;
		this.password = '';
		await this.action.run('pairing', 'enable', async () => {
			this.grant = await enablePairing(password);
			await this.status.load();
		});
	}

	async disable(): Promise<void> {
		await this.action.run('pairing', 'disable', async () => {
			await disablePairing();
			this.grant = null;
			await this.load();
		});
	}

	async revoke(id: string): Promise<void> {
		await this.action.run(id, 'revoke', async () => {
			await revokePairedClient(id);
			await this.clients.load();
		});
	}
}
