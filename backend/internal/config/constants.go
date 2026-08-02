package config

import "time"

// ============================================================================
// HTTP Server Configuration
// ============================================================================

// HTTP Server timeouts
const (
	// HTTPReadTimeout is the maximum duration for reading the entire request
	HTTPReadTimeout = 30 * time.Second

	// HTTPWriteTimeout is the maximum duration before timing out writes of the response
	// Set to 0 to allow long-lived media streams and large uploads/downloads.
	HTTPWriteTimeout = 0 * time.Second

	// HTTPIdleTimeout is the maximum amount of time to wait for the next request
	HTTPIdleTimeout = 120 * time.Second

	// ShutdownTimeout is the maximum time to wait for graceful shutdown
	ShutdownTimeout = 30 * time.Second
)

// ============================================================================
// WebSocket Configuration
// ============================================================================

// WebSocket configuration constants
const (
	// WSWriteWait is the time allowed to write a message to the peer
	WSWriteWait = 10 * time.Second

	// WSPongWait is the time allowed to read the next pong message from the peer
	WSPongWait = 60 * time.Second

	// WSPingPeriod is how often to send pings to peer (must be less than WSPongWait)
	WSPingPeriod = (WSPongWait * 9) / 10

	// WSMaxMessageSize is the maximum message size allowed from peer
	WSMaxMessageSize = 512

	// WSSendBufferSize is the size of the send channel buffer
	WSSendBufferSize = 256

	// WSReadBufferSize is the read buffer size for the WebSocket upgrader
	WSReadBufferSize = 1024

	// WSWriteBufferSize is the write buffer size for the WebSocket upgrader
	WSWriteBufferSize = 1024
)

// ============================================================================
// Job Service Configuration
// ============================================================================

// Job service configuration constants
const (
	// DefaultJobWorkers is the default number of concurrent job workers
	DefaultJobWorkers = 4

	// JobQueueSize is the size of the job work queue channel
	JobQueueSize = 100

	// FileCopyBufferSize is the buffer size for file copy operations (1MB)
	FileCopyBufferSize = 1024 * 1024

	// JobRetentionPeriod is how long to keep completed jobs in memory
	JobRetentionPeriod = 24 * time.Hour

	// JobCleanupInterval is how often to run job cleanup
	JobCleanupInterval = 1 * time.Hour
)

// ============================================================================
// Upload Configuration
// ============================================================================

// Upload configuration constants
const (
	// DefaultMaxUploadMB is the default maximum accepted upload size in megabytes
	DefaultMaxUploadMB = 10240

	// DefaultChunkSizeMB is the default chunk size for uploads in megabytes
	DefaultChunkSizeMB = 10

	// SessionTimeout is how long before an upload session expires due to inactivity
	SessionTimeout = 24 * time.Hour

	// SessionCleanupInterval is how often to run session cleanup
	SessionCleanupInterval = 1 * time.Hour

	// DefaultUploadTempDir is where chunk files are stored before final assembly
	DefaultUploadTempDir = "/tmp/boxbox"
)

// ============================================================================
// Auth Configuration
// ============================================================================

// Auth configuration constants
const (
	// DefaultAccessTokenExpiry is the default access token lifetime
	DefaultAccessTokenExpiry = 15 * time.Minute

	// DefaultRefreshTokenExpiry is the default refresh token lifetime
	DefaultRefreshTokenExpiry = 7 * 24 * time.Hour

	// TokenCleanupInterval is how often to run revoked token cleanup
	TokenCleanupInterval = 1 * time.Hour

	// LoginLockoutThreshold is the number of consecutive failures allowed
	// before per-account exponential backoff begins.
	LoginLockoutThreshold = 5

	// LoginLockoutBaseDelay is the first lockout duration.
	LoginLockoutBaseDelay = 1 * time.Second

	// LoginLockoutMaxDelay caps exponential account lockouts.
	LoginLockoutMaxDelay = 15 * time.Minute
)

// ============================================================================
// Data Storage Configuration
// ============================================================================

// Data storage constants
const (
	// DefaultDataDir is the default directory for persistent data storage
	DefaultDataDir = "/data"

	// DriveNamesFileName is the filename for storing custom drive names
	DriveNamesFileName = "drive-names.json"
)

// ============================================================================
// Filesystem Constants
// ============================================================================

// Filesystem constants
const (
	// ProcMountsPath is the Linux path to the mounts file
	ProcMountsPath = "/proc/mounts"
)

// Disk usage constants
const (
	// PercentMultiplier is used to convert decimal to percentage
	PercentMultiplier = 100
)

// VirtualFilesystems contains filesystem types that should be filtered out during discovery
var VirtualFilesystems = map[string]bool{
	"sysfs": true, "proc": true, "devtmpfs": true, "devpts": true,
	"tmpfs": true, "securityfs": true, "cgroup": true, "cgroup2": true,
	"pstore": true, "debugfs": true, "tracefs": true, "configfs": true,
	"fusectl": true, "mqueue": true, "hugetlbfs": true, "binfmt_misc": true,
	"autofs": true, "overlay": true, "efivarfs": true, "nsfs": true,
	"ramfs": true, "rpc_pipefs": true, "nfsd": true, "squashfs": true,
}

// ExcludedMountPointPrefixes contains mount point path prefixes that should be filtered out
// These are typically Docker bind mounts, system paths, or container-specific mounts
// Also includes /host_root equivalents for when running in Docker
var ExcludedMountPointPrefixes = []string{
	// Container-internal mounts
	"/etc/",           // Docker bind mounts like /etc/hosts, /etc/resolv.conf, /etc/hostname
	"/app/",           // Application config bind mounts
	"/proc/",          // Process filesystem
	"/sys/",           // System filesystem
	"/dev/",           // Device filesystem
	"/run/",           // Runtime data
	"/var/lib/docker", // Docker internal paths
	"/snap/",          // Snap packages
	"/boot/",          // Boot partition
	// Host paths via /host_root bind mount - exclude duplicates and system paths
	"/host_root/etc/",           // Host's etc directory
	"/host_root/proc/",          // Host's proc
	"/host_root/sys/",           // Host's sys
	"/host_root/dev/",           // Host's dev
	"/host_root/run/",           // Host's run
	"/host_root/var/lib/docker", // Host's docker
	"/host_root/snap/",          // Host's snap packages
	"/host_root/boot/",          // Host's boot partition
	// These are duplicates - already accessible via direct mounts
	"/host_root/media/", // Already accessible via /media/devmon mount
	"/host_root/home/",  // Already accessible via /home/user mount
}

// ExcludedMountPointSuffixes contains mount point path suffixes that should be filtered out
var ExcludedMountPointSuffixes = []string{
	"/efivars", // EFI variables
	"/efi",     // EFI partition (small boot partition)
}

// AllowedMountPointPrefixes contains mount point prefixes that should be included
// if they pass the exclusion checks above
var AllowedMountPointPrefixes = []string{
	"/",               // Root filesystem (exact match handled specially)
	"/home",           // User home directories (container)
	"/media/",         // Removable media (container)
	"/mnt/",           // Manual mounts (container)
	"/host_root",      // Host root (exact match - the main host filesystem)
	"/host_root/mnt/", // Host manual mounts (rclone, etc. that aren't in /media)
}
