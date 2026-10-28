import {
	generatePairingCode,
	listPairedClients,
	pairingStatus,
	revokePairedClient,
	type PairingCode,
	type PairedClient
} from '#lib/api/connect.js';

type ClientsState =
	{ kind: 'loading' } | { kind: 'ready'; clients: PairedClient[] } | { kind: 'error' };

type PairingState =
	| { kind: 'idle' }
	| { kind: 'generating' }
	| { kind: 'ready'; pairing: PairingCode }
	| { kind: 'expired' }
	| { kind: 'closed' };

export class AccountPairing {
	password = $state('');
	state = $state<PairingState>({ kind: 'idle' });
	error = $state('');
	copied = $state(false);
	private timer: ReturnType<typeof setTimeout> | undefined;
	private disposed = false;
	clients = $state.raw<ClientsState>({ kind: 'loading' });
	connectionError = $state('');
	revoking = $state<string | null>(null);
	private poll: ReturnType<typeof setInterval> | undefined;
	private request: AbortController | undefined;

	start(): void {
		void this.refresh();
		this.poll = setInterval(() => {
			if (!document.hidden) void this.refresh();
		}, 3000);
	}

	async refresh(): Promise<void> {
		if (this.request || this.disposed) return;
		const request = new AbortController();
		this.request = request;
		const codeState = this.state;
		try {
			const [clients, status] = await Promise.all([
				listPairedClients(request.signal),
				pairingStatus(request.signal)
			]);
			if (this.disposed || request.signal.aborted) return;
			this.clients = { kind: 'ready', clients };
			this.connectionError = '';
			if (
				this.state === codeState &&
				codeState.kind === 'ready' &&
				(!status.enabled || status.expiresAt !== codeState.pairing.expiresAt)
			) {
				clearTimeout(this.timer);
				this.state = { kind: 'closed' };
			}
		} catch (error) {
			if (this.disposed || request.signal.aborted) return;
			if (this.clients.kind === 'loading') this.clients = { kind: 'error' };
			this.connectionError =
				error instanceof Error ? error.message : 'Could not refresh paired devices. Try again.';
		} finally {
			this.request = undefined;
		}
	}

	async revoke(id: string): Promise<void> {
		if (this.revoking) return;
		this.revoking = id;
		this.connectionError = '';
		try {
			await revokePairedClient(id);
			if (this.disposed) return;
			// Ignore any snapshot started before the revocation.
			this.request?.abort();
			if (this.clients.kind === 'ready')
				this.clients = {
					kind: 'ready',
					clients: this.clients.clients.filter((client) => client.clientId !== id)
				};
		} catch (error) {
			if (!this.disposed)
				this.connectionError =
					error instanceof Error ? error.message : 'Could not revoke this device. Try again.';
		} finally {
			if (!this.disposed) this.revoking = null;
		}
	}

	async generate(): Promise<void> {
		if (this.state.kind === 'generating' || !this.password) return;
		clearTimeout(this.timer);
		this.state = { kind: 'generating' };
		this.error = '';
		this.copied = false;
		const password = this.password;
		this.password = '';
		try {
			const pairing = await generatePairingCode(password);
			if (this.disposed) return;
			this.state = { kind: 'ready', pairing };
			this.timer = setTimeout(
				() => {
					this.state = { kind: 'expired' };
				},
				Date.parse(pairing.expiresAt) - Date.now()
			);
		} catch (error) {
			if (this.disposed) return;
			this.state = { kind: 'idle' };
			this.error =
				error instanceof Error ? error.message : 'Could not generate a pairing code. Try again.';
		}
	}

	async copy(): Promise<void> {
		if (this.state.kind !== 'ready') return;
		this.error = '';
		try {
			await navigator.clipboard.writeText(this.state.pairing.code);
			if (!this.disposed && this.state.kind === 'ready') this.copied = true;
		} catch {
			if (!this.disposed) this.error = 'Could not copy. Select and copy the code above.';
		}
	}

	dispose(): void {
		this.disposed = true;
		clearInterval(this.poll);
		this.request?.abort();
		clearTimeout(this.timer);
		this.password = '';
		this.state = { kind: 'idle' };
	}
}
