/**
 * WebSocket store for real-time connection management
 * Requirements: 5.1, 5.4
 */

import { writable, derived, get } from 'svelte/store';
import { getAccessToken } from '$lib/api/client';
import { CONFIG } from '$lib/config';
import { jobsStore, type JobUpdate } from './jobs';

/**
 * WebSocket connection states
 */
export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'reconnecting';

/**
 * WebSocket message types from server
 */
export type ServerMessageType = 'job_update' | 'job_complete' | 'error' | 'pong';

/**
 * WebSocket message from server
 */
export interface WSServerMessage {
	type: ServerMessageType;
	payload: JobUpdate | { message: string };
}

/**
 * WebSocket message types to server
 */
export type ClientMessageType = 'subscribe' | 'unsubscribe' | 'ping';

/**
 * WebSocket message to server
 */
export interface WSClientMessage {
	type: ClientMessageType;
	jobId?: string;
}

/**
 * WebSocket state
 */
export interface WebSocketState {
	connectionState: ConnectionState;
	error: string | null;
	reconnectAttempts: number;
	lastConnectedAt: Date | null;
	subscribedJobs: Set<string>;
}

/**
 * Initial WebSocket state
 */
const initialState: WebSocketState = {
	connectionState: 'disconnected',
	error: null,
	reconnectAttempts: 0,
	lastConnectedAt: null,
	subscribedJobs: new Set()
};

/**
 * Exponential backoff configuration
 */
const BACKOFF_CONFIG = {
	initialDelayMs: CONFIG.websocket.initialReconnectDelayMs,
	maxDelayMs: CONFIG.websocket.maxReconnectDelayMs,
	multiplier: 2,
	maxAttempts: CONFIG.websocket.maxReconnectAttempts
};

/**
 * Ping interval for connection health
 */
const PING_INTERVAL_MS = CONFIG.websocket.pingIntervalMs;

/**
 * Create the WebSocket store
 */
function createWebSocketStore() {
	const { subscribe, set, update } = writable<WebSocketState>(initialState);

	let socket: WebSocket | null = null;
	let reconnectTimeout: ReturnType<typeof setTimeout> | null = null;
	let pingInterval: ReturnType<typeof setInterval> | null = null;

	/**
	 * Get WebSocket URL with auth token
	 */
	function getWebSocketUrl(): string {
		const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
		const token = getAccessToken();
		const baseUrl = `${protocol}//${window.location.host}/api/v1/ws`;
		return token ? `${baseUrl}?token=${encodeURIComponent(token)}` : baseUrl;
	}

	/**
	 * Calculate backoff delay for reconnection
	 */
	function getBackoffDelay(attempt: number): number {
		const delay = BACKOFF_CONFIG.initialDelayMs * Math.pow(BACKOFF_CONFIG.multiplier, attempt);
		return Math.min(delay, BACKOFF_CONFIG.maxDelayMs);
	}

	/**
	 * Start ping interval for connection health
	 */
	function startPingInterval(): void {
		stopPingInterval();
		pingInterval = setInterval(() => {
			if (socket?.readyState === WebSocket.OPEN) {
				sendMessage({ type: 'ping' });
			}
		}, PING_INTERVAL_MS);
	}

	/**
	 * Stop ping interval
	 */
	function stopPingInterval(): void {
		if (pingInterval) {
			clearInterval(pingInterval);
			pingInterval = null;
		}
	}

	/**
	 * Clear reconnect timeout
	 */
	function clearReconnectTimeout(): void {
		if (reconnectTimeout) {
			clearTimeout(reconnectTimeout);
			reconnectTimeout = null;
		}
	}

	/**
	 * Handle one parsed WebSocket server message
	 */
	function handleServerMessage(message: WSServerMessage): void {
		switch (message.type) {
			case 'job_update':
			case 'job_complete':
				// Update jobs store with the job update
				jobsStore.updateFromWebSocket(message.payload as JobUpdate);
				break;

			case 'error': {
				const errorPayload = message.payload as { message: string };
				update((state) => ({
					...state,
					error: errorPayload.message
				}));
				break;
			}

			case 'pong':
				// Connection is healthy, nothing to do
				break;

			default:
				console.warn('Unknown WebSocket message type:', message.type);
		}
	}

	/**
	 * Parse one text WebSocket frame. The backend may batch multiple JSON
	 * messages in a single frame, separated by newlines.
	 */
	function handleMessageText(data: string): void {
		for (const line of data.split('\n')) {
			const text = line.trim();
			if (!text) {
				continue;
			}

			try {
				handleServerMessage(JSON.parse(text) as WSServerMessage);
			} catch (err) {
				console.error('Failed to parse WebSocket message:', err);
			}
		}
	}

	/**
	 * Handle incoming WebSocket message
	 */
	function handleMessage(event: MessageEvent): void {
		if (typeof event.data === 'string') {
			handleMessageText(event.data);
			return;
		}

		if (event.data instanceof Blob) {
			event.data
				.text()
				.then(handleMessageText)
				.catch((err) => console.error('Failed to read WebSocket message:', err));
		}
	}

	/**
	 * Connect to WebSocket server
	 */
	function connect(): void {
		// Don't connect if already connected or connecting
		const currentState = get({ subscribe });
		if (
			currentState.connectionState === 'connected' ||
			currentState.connectionState === 'connecting'
		) {
			return;
		}

		// Check for auth token
		const token = getAccessToken();
		if (!token) {
			update((state) => ({
				...state,
				connectionState: 'disconnected',
				error: 'Not authenticated'
			}));
			return;
		}

		update((state) => ({
			...state,
			connectionState: 'connecting',
			error: null
		}));

		try {
			socket = new WebSocket(getWebSocketUrl());

			socket.onopen = () => {
				update((state) => ({
					...state,
					connectionState: 'connected',
					error: null,
					reconnectAttempts: 0,
					lastConnectedAt: new Date()
				}));
				startPingInterval();

				// Re-subscribe to any jobs we were subscribed to
				const state = get({ subscribe });
				for (const jobId of state.subscribedJobs) {
					sendMessage({ type: 'subscribe', jobId });
				}
			};

			socket.onclose = (event) => {
				stopPingInterval();
				socket = null;

				// Don't reconnect if closed cleanly (code 1000) or if we're disconnecting intentionally
				if (event.code === 1000) {
					update((state) => ({
						...state,
						connectionState: 'disconnected'
					}));
					return;
				}

				// Attempt reconnection with exponential backoff
				const state = get({ subscribe });
				if (state.reconnectAttempts < BACKOFF_CONFIG.maxAttempts) {
					const delay = getBackoffDelay(state.reconnectAttempts);
					update((s) => ({
						...s,
						connectionState: 'reconnecting',
						reconnectAttempts: s.reconnectAttempts + 1
					}));

					reconnectTimeout = setTimeout(() => {
						connect();
					}, delay);
				} else {
					update((s) => ({
						...s,
						connectionState: 'disconnected',
						error: 'Max reconnection attempts reached'
					}));
				}
			};

			socket.onerror = () => {
				update((state) => ({
					...state,
					error: 'WebSocket connection error'
				}));
			};

			socket.onmessage = handleMessage;
		} catch (err) {
			update((state) => ({
				...state,
				connectionState: 'disconnected',
				error: err instanceof Error ? err.message : 'Failed to connect'
			}));
		}
	}

	/**
	 * Disconnect from WebSocket server
	 */
	function disconnect(): void {
		clearReconnectTimeout();
		stopPingInterval();

		if (socket) {
			socket.close(1000, 'Client disconnecting');
			socket = null;
		}

		set(initialState);
	}

	/**
	 * Send a message to the server
	 */
	function sendMessage(message: WSClientMessage): boolean {
		if (socket?.readyState === WebSocket.OPEN) {
			socket.send(JSON.stringify(message));
			return true;
		}
		return false;
	}

	/**
	 * Subscribe to job updates
	 */
	function subscribeToJob(jobId: string): void {
		update((state) => {
			const newSubscribed = new Set(state.subscribedJobs);
			newSubscribed.add(jobId);
			return { ...state, subscribedJobs: newSubscribed };
		});

		sendMessage({ type: 'subscribe', jobId });
	}

	/**
	 * Unsubscribe from job updates
	 */
	function unsubscribeFromJob(jobId: string): void {
		update((state) => {
			const newSubscribed = new Set(state.subscribedJobs);
			newSubscribed.delete(jobId);
			return { ...state, subscribedJobs: newSubscribed };
		});

		sendMessage({ type: 'unsubscribe', jobId });
	}

	/**
	 * Reconcile job subscriptions with the provided active job IDs.
	 */
	function syncJobSubscriptions(jobIds: string[]): void {
		const desired = new Set(jobIds);
		const current = get({ subscribe }).subscribedJobs;

		for (const jobId of desired) {
			if (!current.has(jobId)) {
				subscribeToJob(jobId);
			}
		}

		for (const jobId of current) {
			if (!desired.has(jobId)) {
				unsubscribeFromJob(jobId);
			}
		}
	}

	/**
	 * Check if connected
	 */
	function isConnected(): boolean {
		return get({ subscribe }).connectionState === 'connected';
	}

	/**
	 * Clear error
	 */
	function clearError(): void {
		update((state) => ({ ...state, error: null }));
	}

	/**
	 * Force reconnect (reset attempts and connect)
	 */
	function forceReconnect(): void {
		disconnect();
		update((state) => ({ ...state, reconnectAttempts: 0 }));
		connect();
	}

	return {
		subscribe,
		connect,
		disconnect,
		sendMessage,
		subscribeToJob,
		unsubscribeFromJob,
		syncJobSubscriptions,
		isConnected,
		clearError,
		forceReconnect
	};
}

/**
 * WebSocket store singleton
 */
export const websocketStore = createWebSocketStore();

/**
 * Derived store for connection state
 */
export const connectionState = derived(websocketStore, ($ws) => $ws.connectionState);

/**
 * Derived store for whether connected
 */
export const isConnected = derived(websocketStore, ($ws) => $ws.connectionState === 'connected');

/**
 * Derived store for whether reconnecting
 */
export const isReconnecting = derived(
	websocketStore,
	($ws) => $ws.connectionState === 'reconnecting'
);

/**
 * Derived store for WebSocket error
 */
export const websocketError = derived(websocketStore, ($ws) => $ws.error);

/**
 * Derived store for reconnect attempts
 */
export const reconnectAttempts = derived(websocketStore, ($ws) => $ws.reconnectAttempts);
