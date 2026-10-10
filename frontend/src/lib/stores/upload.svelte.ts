/**
 * Upload store using Svelte 5 runes
 * Manages sequential file uploads with progress tracking
 */

import type { UploadProgress, UploadOptions } from '$lib/utils/upload';
import { generateUploadId, getChunkCount } from '$lib/utils/uploadIds';
import { getUploadConfig } from '$lib/api/system';
import { CONFIG } from '$lib/config';
import type { UploadEntry } from '$lib/utils/uploadEntries';

export type { UploadProgress };

/**
 * Upload queue item
 */
interface QueueItem {
	file: File;
	destPath: string;
	uploadId: string;
	/** Name shown in progress: the path below the destination. */
	displayName: string;
}

/**
 * Upload store class using Svelte 5 runes
 * Handles sequential uploads (one at a time)
 */
class UploadStore {
	/** Current uploads with their progress */
	uploads = $state<UploadProgress[]>([]);

	/** Whether an upload is currently in progress */
	isUploading = $state(false);

	/** Queue for pending uploads */
	private queue: QueueItem[] = [];

	/** Active uploads and their cancellation controllers. */
	private controllers = new Map<string, AbortController>();
	private activeWorkers = 0;
	private refreshPending = false;

	/** Server-configured chunk size, loaded before the first upload starts. */
	private chunkSize = CONFIG.upload.defaultChunkSize;
	private chunkSizeLoaded = false;
	private chunkSizeRequest: Promise<void> | null = null;

	/** Callback for when upload completes */
	onComplete?: (fileName: string, success: boolean, error?: string) => void;

	/** Callback for when directory should refresh */
	onRefreshNeeded?: () => void;

	/**
	 * Derived: whether there are any uploads (active or completed)
	 */
	get hasUploads(): boolean {
		return this.uploads.length > 0;
	}

	/**
	 * Derived: count of active (pending/uploading) uploads
	 */
	get activeCount(): number {
		return this.uploads.filter((u) => u.status === 'pending' || u.status === 'uploading').length;
	}

	/**
	 * Derived: count of completed/error/cancelled uploads
	 */
	get completedCount(): number {
		return this.uploads.filter(
			(u) => u.status === 'complete' || u.status === 'error' || u.status === 'cancelled'
		).length;
	}

	/**
	 * Add files to the upload queue. Folder uploads keep each file's relative path;
	 * the server creates the missing folders.
	 * @param entries Files with their path below the destination
	 * @param destPath Destination directory path (virtual path like "media/movies")
	 */
	addFiles(entries: UploadEntry[], destPath: string): void {
		for (const { file, relativePath } of entries) {
			const uploadId = generateUploadId();
			const filePath = destPath ? `${destPath}/${relativePath}` : relativePath;

			// Add to queue
			this.queue.push({ file, destPath: filePath, uploadId, displayName: relativePath });

			// Add initial progress entry
			const progress: UploadProgress = {
				uploadId,
				fileName: relativePath,
				totalSize: file.size,
				uploadedSize: 0,
				percentage: 0,
				currentChunk: 0,
				totalChunks: getChunkCount(file.size, this.chunkSize),
				status: 'pending'
			};

			this.uploads = [...this.uploads, progress];
		}

		// Start processing queue if not already
		this.processQueue();
	}

	/**
	 * Fill the bounded upload worker pool.
	 */
	private processQueue(): void {
		while (this.activeWorkers < CONFIG.upload.maxConcurrentUploads && this.queue.length > 0) {
			const item = this.queue.shift()!;

			// Check if this upload was cancelled before starting
			const existingProgress = this.uploads.find((u) => u.uploadId === item.uploadId);
			if (existingProgress?.status === 'cancelled') {
				continue;
			}

			this.activeWorkers++;
			this.isUploading = true;
			void this.processItem(item);
		}
	}

	/**
	 * Workers that start together share one request, so they all use the same chunk size.
	 * It keeps the default when the server cannot be reached and asks again on the next upload.
	 */
	private loadChunkSize(): Promise<void> {
		if (this.chunkSizeLoaded) return Promise.resolve();
		this.chunkSizeRequest ??= getUploadConfig()
			.then((config) => {
				this.chunkSize = config.chunkSizeBytes;
				this.chunkSizeLoaded = true;
			})
			.catch(() => {})
			.finally(() => {
				this.chunkSizeRequest = null;
			});
		return this.chunkSizeRequest;
	}

	private async processItem(item: QueueItem): Promise<void> {
		const controller = new AbortController();
		this.controllers.set(item.uploadId, controller);

		try {
			await this.loadChunkSize();
			// Cancelled while waiting: leave the cancelled state as it is.
			if (controller.signal.aborted) return;

			const options: UploadOptions = {
				uploadId: item.uploadId,
				chunkSize: this.chunkSize,
				signal: controller.signal,
				onProgress: (progress) => {
					this.updateProgress(item.uploadId, progress);
				}
			};
			// The transfer and hashing code loads with the first upload, not with the page.
			const { resumeUpload } = await import('$lib/utils/upload');
			if (controller.signal.aborted) return;
			const result = await resumeUpload(item.file, item.destPath, item.uploadId, options);

			if (result.success) {
				const totalChunks = getChunkCount(item.file.size, this.chunkSize);
				this.updateProgress(item.uploadId, {
					uploadId: item.uploadId,
					fileName: item.displayName,
					totalSize: item.file.size,
					uploadedSize: item.file.size,
					percentage: 100,
					currentChunk: totalChunks,
					totalChunks,
					status: 'complete'
				});
				this.refreshPending = true;
				this.onComplete?.(item.displayName, true);
			} else {
				this.markFailed(item, result.error || 'Upload failed');
			}
		} catch (err) {
			this.markFailed(item, err instanceof Error ? err.message : 'Upload failed');
		} finally {
			this.controllers.delete(item.uploadId);
			this.activeWorkers--;
			this.processQueue();
			if (this.activeWorkers === 0 && this.queue.length === 0) {
				this.isUploading = false;
				if (this.refreshPending) {
					this.refreshPending = false;
					this.onRefreshNeeded?.();
				}
			}
		}
	}

	private markFailed(item: QueueItem, error: string): void {
		const upload = this.uploads.find((candidate) => candidate.uploadId === item.uploadId);
		if (!upload || upload.status === 'cancelled') {
			return;
		}
		this.updateProgress(item.uploadId, {
			uploadId: item.uploadId,
			fileName: item.displayName,
			totalSize: item.file.size,
			uploadedSize: 0,
			percentage: 0,
			currentChunk: 0,
			totalChunks: getChunkCount(item.file.size, this.chunkSize),
			status: 'error',
			error
		});
		this.onComplete?.(item.displayName, false, error);
	}

	/**
	 * Update progress for an upload
	 */
	private updateProgress(uploadId: string, progress: UploadProgress): void {
		// Keep the queued display name (the relative path for folder uploads).
		this.uploads = this.uploads.map((u) =>
			u.uploadId === uploadId ? { ...progress, fileName: u.fileName } : u
		);
	}

	/**
	 * Cancel an upload
	 */
	cancel(uploadId: string): void {
		// If it's the current upload, abort it
		this.controllers.get(uploadId)?.abort();

		// Remove from queue if pending
		this.queue = this.queue.filter((q) => q.uploadId !== uploadId);

		// Update status
		this.uploads = this.uploads.map((u) =>
			u.uploadId === uploadId && (u.status === 'pending' || u.status === 'uploading')
				? { ...u, status: 'cancelled' as const }
				: u
		);
	}

	/**
	 * Remove an upload from the list (only for completed/error/cancelled)
	 */
	remove(uploadId: string): void {
		const upload = this.uploads.find((u) => u.uploadId === uploadId);
		if (
			upload &&
			(upload.status === 'complete' || upload.status === 'error' || upload.status === 'cancelled')
		) {
			this.uploads = this.uploads.filter((u) => u.uploadId !== uploadId);
		}
	}

	/**
	 * Clear all completed/error/cancelled uploads
	 */
	clearFinished(): void {
		this.uploads = this.uploads.filter((u) => u.status === 'pending' || u.status === 'uploading');
	}

	/**
	 * Clear all uploads and cancel any in progress
	 */
	clearAll(): void {
		// Cancel current upload
		for (const controller of this.controllers.values()) controller.abort();
		this.controllers.clear();

		// Clear queue
		this.queue = [];

		// Clear all uploads
		this.uploads = [];
		this.isUploading = this.activeWorkers > 0;
		this.refreshPending = false;
	}
}

/**
 * Singleton upload store instance
 */
export const uploadStore = new UploadStore();
