/**
 * WebSocket connection that carries job progress from the server
 */

import { untrack } from 'svelte';
import { getAccessToken } from '$lib/api/client';
import { parseServerMessage, type WSServerMessage } from '$lib/api/websocket';
import { CONFIG } from '$lib/config';
import { jobsStore } from './jobs.svelte';

export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'reconnecting';

type WSClientMessage = { type: 'ping' } | { type: 'subscribe' | 'unsubscribe'; jobId: string };

const BACKOFF = {
	initialDelayMs: CONFIG.websocket.initialReconnectDelayMs,
	maxDelayMs: CONFIG.websocket.maxReconnectDelayMs,
	maxAttempts: CONFIG.websocket.maxReconnectAttempts
};

function webSocketUrl(): string {
	const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
	const token = getAccessToken();
	const baseUrl = `${protocol}//${window.location.host}/api/v1/ws`;
	return token ? `${baseUrl}?token=${encodeURIComponent(token)}` : baseUrl;
}

class WebSocketStore {
	connectionState = $state<ConnectionState>('disconnected');
	error = $state<string | null>(null);

	private socket: WebSocket | null = null;
	private reconnectTimeout: ReturnType<typeof setTimeout> | null = null;
	private pingInterval: ReturnType<typeof setInterval> | null = null;
	private reconnectAttempts = 0;
	/** Retries reuse the mode of the last connect(); a --dev server has no token to send. */
	private allowUnauthenticated = false;
	private subscribedJobs = new Set<string>();

	connect(allowUnauthenticated = false): void {
		// An effect calls this. It must not depend on the state it changes, or every
		// state change would reconnect at once and skip the backoff delay.
		const state = untrack(() => this.connectionState);
		if (state === 'connected' || state === 'connecting') return;

		this.allowUnauthenticated = allowUnauthenticated;

		if (!getAccessToken() && !allowUnauthenticated) {
			this.connectionState = 'disconnected';
			this.error = 'Not authenticated';
			return;
		}

		this.connectionState = 'connecting';
		this.error = null;

		try {
			const socket = new WebSocket(webSocketUrl());
			this.socket = socket;
			socket.onopen = () => this.handleOpen();
			socket.onclose = (event) => this.handleClose(event);
			socket.onerror = () => {
				this.error = 'WebSocket connection error';
			};
			socket.onmessage = (event) => this.handleMessage(event);
		} catch (err) {
			this.connectionState = 'disconnected';
			this.error = err instanceof Error ? err.message : 'Failed to connect';
		}
	}

	disconnect(): void {
		this.clearReconnectTimeout();
		this.stopPing();

		if (this.socket) {
			this.socket.close(1000, 'Client disconnecting');
			this.socket = null;
		}

		this.connectionState = 'disconnected';
		this.error = null;
		this.reconnectAttempts = 0;
		this.subscribedJobs = new Set();
	}

	/** Subscribes to the given jobs and drops subscriptions for any other job. */
	syncJobSubscriptions(jobIds: string[]): void {
		const desired = new Set(jobIds);

		for (const jobId of desired) {
			if (!this.subscribedJobs.has(jobId)) {
				this.subscribedJobs.add(jobId);
				this.send({ type: 'subscribe', jobId });
			}
		}
		for (const jobId of this.subscribedJobs) {
			if (!desired.has(jobId)) {
				this.subscribedJobs.delete(jobId);
				this.send({ type: 'unsubscribe', jobId });
			}
		}
	}

	private send(message: WSClientMessage): void {
		if (this.socket?.readyState === WebSocket.OPEN) {
			this.socket.send(JSON.stringify(message));
		}
	}

	private handleOpen(): void {
		// Updates sent while the socket was down are lost, so ask the server where the jobs stand now.
		if (this.reconnectAttempts > 0) void jobsStore.loadJobs();
		this.connectionState = 'connected';
		this.error = null;
		this.reconnectAttempts = 0;

		this.stopPing();
		this.pingInterval = setInterval(
			() => this.send({ type: 'ping' }),
			CONFIG.websocket.pingIntervalMs
		);

		for (const jobId of this.subscribedJobs) {
			this.send({ type: 'subscribe', jobId });
		}
	}

	private handleClose(event: CloseEvent): void {
		this.stopPing();
		this.socket = null;

		if (event.code === 1000) {
			this.connectionState = 'disconnected';
			return;
		}

		if (this.reconnectAttempts >= BACKOFF.maxAttempts) {
			this.connectionState = 'disconnected';
			this.error = 'Max reconnection attempts reached';
			return;
		}

		const delay = Math.min(
			BACKOFF.initialDelayMs * 2 ** this.reconnectAttempts,
			BACKOFF.maxDelayMs
		);
		this.reconnectAttempts += 1;
		this.connectionState = 'reconnecting';
		this.reconnectTimeout = setTimeout(() => this.connect(this.allowUnauthenticated), delay);
	}

	/** The server may batch several JSON messages in one frame, one per line. */
	private handleMessageText(data: string): void {
		for (const line of data.split('\n')) {
			const text = line.trim();
			if (!text) continue;

			const message = parseServerMessage(text);
			if (message) {
				this.handleServerMessage(message);
			} else {
				console.warn('Ignoring unrecognised WebSocket message');
			}
		}
	}

	private handleServerMessage(message: WSServerMessage): void {
		switch (message.type) {
			case 'job_update':
			case 'job_complete':
				jobsStore.updateFromWebSocket(message.payload);
				break;
			case 'error':
				this.error = message.payload.message;
				break;
			case 'pong':
				break;
			default:
				message satisfies never;
		}
	}

	private handleMessage(event: MessageEvent): void {
		if (typeof event.data === 'string') {
			this.handleMessageText(event.data);
		} else if (event.data instanceof Blob) {
			event.data
				.text()
				.then((text) => this.handleMessageText(text))
				.catch((err) => console.error('Failed to read WebSocket message:', err));
		}
	}

	private stopPing(): void {
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
}

export const websocketStore = new WebSocketStore();
