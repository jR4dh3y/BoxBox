/**
 * Upload store using Svelte 5 runes
 * Manages sequential file uploads with progress tracking
 */

import {
	resumeUpload,
	type UploadProgress,
	type UploadOptions,
	generateUploadId,
	getChunkCount
} from '$lib/utils/upload';
import { CONFIG } from '$lib/config';

export type { UploadProgress };

/**
 * Upload queue item
 */
interface QueueItem {
	file: File;
	destPath: string;
	uploadId: string;
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
	 * Add files to the upload queue
	 * @param files Files to upload
	 * @param destPath Destination directory path (virtual path like "media/movies")
	 */
	addFiles(files: File[], destPath: string): void {
		for (const file of files) {
			const uploadId = generateUploadId();
			const filePath = destPath ? `${destPath}/${file.name}` : file.name;

			// Add to queue
			this.queue.push({ file, destPath: filePath, uploadId });

			// Add initial progress entry
			const progress: UploadProgress = {
				uploadId,
				fileName: file.name,
				totalSize: file.size,
				uploadedSize: 0,
				percentage: 0,
				currentChunk: 0,
				totalChunks: getChunkCount(file.size),
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

	private async processItem(item: QueueItem): Promise<void> {
		const controller = new AbortController();
		this.controllers.set(item.uploadId, controller);

		const options: UploadOptions = {
			uploadId: item.uploadId,
			signal: controller.signal,
			onProgress: (progress) => {
				this.updateProgress(item.uploadId, progress);
			}
		};

		try {
			const result = await resumeUpload(item.file, item.destPath, item.uploadId, options);

			if (result.success) {
				const totalChunks = getChunkCount(item.file.size);
				this.updateProgress(item.uploadId, {
					uploadId: item.uploadId,
					fileName: item.file.name,
					totalSize: item.file.size,
					uploadedSize: item.file.size,
					percentage: 100,
					currentChunk: totalChunks,
					totalChunks,
					status: 'complete'
				});
				this.refreshPending = true;
				this.onComplete?.(item.file.name, true);
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
			fileName: item.file.name,
			totalSize: item.file.size,
			uploadedSize: 0,
			percentage: 0,
			currentChunk: 0,
			totalChunks: getChunkCount(item.file.size),
			status: 'error',
			error
		});
		this.onComplete?.(item.file.name, false, error);
	}

	/**
	 * Update progress for an upload
	 */
	private updateProgress(uploadId: string, progress: UploadProgress): void {
		this.uploads = this.uploads.map((u) => (u.uploadId === uploadId ? { ...progress } : u));
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
