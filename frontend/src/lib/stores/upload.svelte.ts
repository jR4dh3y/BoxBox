/**
 * Upload store using Svelte 5 runes
 * Manages sequential file uploads with progress tracking
 */

import {
	uploadFile,
	type UploadProgress,
	type UploadOptions,
	generateUploadId
} from '$lib/utils/upload';

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

	/** Current abort controller for cancellation */
	private currentAbortController: AbortController | null = null;

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
				totalChunks: Math.ceil(file.size / (10 * 1024 * 1024)), // 10MB chunks
				status: 'pending'
			};

			this.uploads = [...this.uploads, progress];
		}

		// Start processing queue if not already
		this.processQueue();
	}

	/**
	 * Process the upload queue sequentially
	 */
	private async processQueue(): Promise<void> {
		if (this.isUploading || this.queue.length === 0) {
			return;
		}

		this.isUploading = true;

		while (this.queue.length > 0) {
			const item = this.queue.shift()!;

			// Check if this upload was cancelled before starting
			const existingProgress = this.uploads.find((u) => u.uploadId === item.uploadId);
			if (existingProgress?.status === 'cancelled') {
				continue;
			}

			// Create abort controller for this upload
			this.currentAbortController = new AbortController();

			const options: UploadOptions = {
				signal: this.currentAbortController.signal,
				onProgress: (progress) => {
					this.updateProgress(item.uploadId, progress);
				}
			};

			try {
				const result = await uploadFile(item.file, item.destPath, options);

				if (result.success) {
					this.updateProgress(item.uploadId, {
						uploadId: item.uploadId,
						fileName: item.file.name,
						totalSize: item.file.size,
						uploadedSize: item.file.size,
						percentage: 100,
						currentChunk: Math.ceil(item.file.size / (10 * 1024 * 1024)),
						totalChunks: Math.ceil(item.file.size / (10 * 1024 * 1024)),
						status: 'complete'
					});
					this.onComplete?.(item.file.name, true);
					this.onRefreshNeeded?.();
				} else {
					this.updateProgress(item.uploadId, {
						uploadId: item.uploadId,
						fileName: item.file.name,
						totalSize: item.file.size,
						uploadedSize: 0,
						percentage: 0,
						currentChunk: 0,
						totalChunks: Math.ceil(item.file.size / (10 * 1024 * 1024)),
						status: 'error',
						error: result.error
					});
					this.onComplete?.(item.file.name, false, result.error);
				}
			} catch (err) {
				const errorMessage = err instanceof Error ? err.message : 'Upload failed';
				this.updateProgress(item.uploadId, {
					uploadId: item.uploadId,
					fileName: item.file.name,
					totalSize: item.file.size,
					uploadedSize: 0,
					percentage: 0,
					currentChunk: 0,
					totalChunks: Math.ceil(item.file.size / (10 * 1024 * 1024)),
					status: 'error',
					error: errorMessage
				});
				this.onComplete?.(item.file.name, false, errorMessage);
			}

			this.currentAbortController = null;
		}

		this.isUploading = false;
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
		const currentUpload = this.uploads.find(
			(u) => u.uploadId === uploadId && u.status === 'uploading'
		);
		if (currentUpload && this.currentAbortController) {
			this.currentAbortController.abort();
		}

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
		if (this.currentAbortController) {
			this.currentAbortController.abort();
		}

		// Clear queue
		this.queue = [];

		// Clear all uploads
		this.uploads = [];
		this.isUploading = false;
	}
}

/**
 * Singleton upload store instance
 */
export const uploadStore = new UploadStore();
