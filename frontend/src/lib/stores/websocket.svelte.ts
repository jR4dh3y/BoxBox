/**
 * WebSocket store for real-time connection management using Svelte 5 runes.
 */

import { getAccessToken } from '$lib/api/client';
import { jobsStore, type JobUpdate } from './jobs.svelte';
import { CONFIG } from '$lib/config';
import { SvelteDate, SvelteSet } from 'svelte/reactivity';

export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'reconnecting';

export type ServerMessageType = 'job_update' | 'job_complete' | 'error' | 'pong';

export interface WSServerMessage {
	type: ServerMessageType;
	payload: JobUpdate | { message: string };
}

export type ClientMessageType = 'subscribe' | 'unsubscribe' | 'ping';

export interface WSClientMessage {
	type: ClientMessageType;
	jobId?: string;
}

export interface WebSocketState {
	connectionState: ConnectionState;
	error: string | null;
	reconnectAttempts: number;
	lastConnectedAt: Date | null;
	subscribedJobs: SvelteSet<string>;
}

const initialState: WebSocketState = {
	connectionState: 'disconnected',
	error: null,
	reconnectAttempts: 0,
	lastConnectedAt: null,
	subscribedJobs: new SvelteSet()
};

const BACKOFF_CONFIG = {
	initialDelayMs: CONFIG.websocket.initialReconnectDelayMs,
	maxDelayMs: CONFIG.websocket.maxReconnectDelayMs,
	multiplier: 2,
	maxAttempts: CONFIG.websocket.maxReconnectAttempts
};

const PING_INTERVAL_MS = CONFIG.websocket.pingIntervalMs;

class WebSocketStore {
	private state = $state<WebSocketState>({
		connectionState: initialState.connectionState,
		error: initialState.error,
		reconnectAttempts: initialState.reconnectAttempts,
		lastConnectedAt: initialState.lastConnectedAt,
		subscribedJobs: new SvelteSet(initialState.subscribedJobs)
	});

	private socket: WebSocket | null = null;
	private reconnectTimeout: ReturnType<typeof setTimeout> | null = null;
	private pingInterval: ReturnType<typeof setInterval> | null = null;

	get connectionState(): ConnectionState {
		return this.state.connectionState;
	}

	get error(): string | null {
		return this.state.error;
	}

	get reconnectAttempts(): number {
		return this.state.reconnectAttempts;
	}

	get connected(): boolean {
		return this.state.connectionState === 'connected';
	}

	get reconnecting(): boolean {
		return this.state.connectionState === 'reconnecting';
	}

	private getWebSocketUrl(): string {
		const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
		const token = getAccessToken();
		const baseUrl = `${protocol}//${window.location.host}/api/v1/ws`;
		return token ? `${baseUrl}?token=${encodeURIComponent(token)}` : baseUrl;
	}

	private getBackoffDelay(attempt: number): number {
		const delay = BACKOFF_CONFIG.initialDelayMs * Math.pow(BACKOFF_CONFIG.multiplier, attempt);
		return Math.min(delay, BACKOFF_CONFIG.maxDelayMs);
	}

	private startPingInterval(): void {
		this.stopPingInterval();
		this.pingInterval = setInterval(() => {
			if (this.socket?.readyState === WebSocket.OPEN) {
				this.sendMessage({ type: 'ping' });
			}
		}, PING_INTERVAL_MS);
	}

	private stopPingInterval(): void {
		if (this.pingInterval) {
			clearInterval(this.pingInterval);
			this.pingInterval = null;
		}
	}

	private clearReconnectTimeout(): void {
		if (this.reconnectTimeout) {
			clearTimeout(this.reconnectTimeout);
			this.reconnectTimeout = null;
		}
	}

	private handleMessage(event: MessageEvent): void {
		try {
			const message: WSServerMessage = JSON.parse(event.data);

			switch (message.type) {
				case 'job_update':
				case 'job_complete':
					jobsStore.updateFromWebSocket(message.payload as JobUpdate);
					break;

				case 'error': {
					const errorPayload = message.payload as { message: string };
					this.state.error = errorPayload.message;
					break;
				}

				case 'pong':
					break;

				default:
					console.warn('Unknown WebSocket message type:', message.type);
			}
		} catch (err) {
			console.error('Failed to parse WebSocket message:', err);
		}
	}

	connect(): void {
		if (this.state.connectionState === 'connected' || this.state.connectionState === 'connecting') {
			return;
		}

		const token = getAccessToken();
		if (!token) {
			this.state.connectionState = 'disconnected';
			this.state.error = 'Not authenticated';
			return;
		}

		this.state.connectionState = 'connecting';
		this.state.error = null;

		try {
			this.socket = new WebSocket(this.getWebSocketUrl());

			this.socket.onopen = () => {
				this.state.connectionState = 'connected';
				this.state.error = null;
				this.state.reconnectAttempts = 0;
				this.state.lastConnectedAt = new SvelteDate();
				this.startPingInterval();

				for (const jobId of this.state.subscribedJobs) {
					this.sendMessage({ type: 'subscribe', jobId });
				}
			};

			this.socket.onclose = (event) => {
				this.stopPingInterval();
				this.socket = null;

				if (event.code === 1000) {
					this.state.connectionState = 'disconnected';
					return;
				}

				if (this.state.reconnectAttempts < BACKOFF_CONFIG.maxAttempts) {
					const delay = this.getBackoffDelay(this.state.reconnectAttempts);
					this.state.connectionState = 'reconnecting';
					this.state.reconnectAttempts += 1;

					this.reconnectTimeout = setTimeout(() => {
						this.connect();
					}, delay);
				} else {
					this.state.connectionState = 'disconnected';
					this.state.error = 'Max reconnection attempts reached';
				}
			};

			this.socket.onerror = () => {
				this.state.error = 'WebSocket connection error';
			};

			this.socket.onmessage = (event) => this.handleMessage(event);
		} catch (err) {
			this.state.connectionState = 'disconnected';
			this.state.error = err instanceof Error ? err.message : 'Failed to connect';
		}
	}

	disconnect(): void {
		this.clearReconnectTimeout();
		this.stopPingInterval();

		if (this.socket) {
			this.socket.close(1000, 'Client disconnecting');
			this.socket = null;
		}

		this.state = {
			connectionState: initialState.connectionState,
			error: initialState.error,
			reconnectAttempts: initialState.reconnectAttempts,
			lastConnectedAt: initialState.lastConnectedAt,
			subscribedJobs: new SvelteSet(initialState.subscribedJobs)
		};
	}

	sendMessage(message: WSClientMessage): boolean {
		if (this.socket?.readyState === WebSocket.OPEN) {
			this.socket.send(JSON.stringify(message));
			return true;
		}
		return false;
	}

	subscribeToJob(jobId: string): void {
		const nextSubscribed = new SvelteSet(this.state.subscribedJobs);
		nextSubscribed.add(jobId);
		this.state.subscribedJobs = nextSubscribed;
		this.sendMessage({ type: 'subscribe', jobId });
	}

	unsubscribeFromJob(jobId: string): void {
		const nextSubscribed = new SvelteSet(this.state.subscribedJobs);
		nextSubscribed.delete(jobId);
		this.state.subscribedJobs = nextSubscribed;
		this.sendMessage({ type: 'unsubscribe', jobId });
	}

	isConnected(): boolean {
		return this.state.connectionState === 'connected';
	}

	clearError(): void {
		this.state.error = null;
	}

	forceReconnect(): void {
		this.disconnect();
		this.state.reconnectAttempts = 0;
		this.connect();
	}
}

export const websocketStore = new WebSocketStore();
