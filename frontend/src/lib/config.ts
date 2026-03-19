/**
 * Centralized configuration constants
 * Single source of truth for all magic numbers and configuration values
 */

export const CONFIG = {
	auth: {
		/** Token refresh interval - refresh 1 minute before 15-min token expiry */
		tokenRefreshIntervalMs: 14 * 60 * 1000,
		/** Access token expiry time */
		accessTokenExpiryMs: 15 * 60 * 1000
	},
	upload: {
		/** Default chunk size for uploads (10MB) */
		defaultChunkSize: 10 * 1024 * 1024,
		/** Maximum concurrent uploads */
		maxConcurrentUploads: 3
	},
	query: {
		/** Default stale time for queries (1 minute) */
		staleTimeMs: 60 * 1000,
		/** Jobs refetch interval (5 seconds) */
		jobsRefetchIntervalMs: 5000
	},
	websocket: {
		/** Ping interval to keep connection alive */
		pingIntervalMs: 30 * 1000,
		/** Maximum reconnection attempts */
		maxReconnectAttempts: 10,
		/** Initial delay before reconnecting */
		initialReconnectDelayMs: 1000,
		/** Maximum delay between reconnection attempts */
		maxReconnectDelayMs: 30 * 1000
	},
	ui: {
		/** Default page size for file listings */
		defaultPageSize: 50,
		/** Debounce delay for search input */
		searchDebounceMs: 300
	},
	paths: {
		/** Mount point name that maps to the host root filesystem in Docker */
		hostRootMount: 'root',
		/**
		 * Mappings from system mount points (as returned by df) to browsable paths.
		 * Order matters - first match wins, so more specific paths should come first.
		 */
		mountPointMappings: [
			{ systemPath: '/media/devmon', browsePath: 'drives' },
			{ systemPath: '/home/user', browsePath: 'home' },
			{ systemPath: '/host_root', browsePath: 'root' }
		] as const
	}
} as const;

/** Storage keys for localStorage */
export const STORAGE_KEYS = {
	ACCESS_TOKEN: 'accessToken',
	REFRESH_TOKEN: 'refreshToken',
	SETTINGS: 'filemanager_settings'
} as const;
