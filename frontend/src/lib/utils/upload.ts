/**
 * Chunked upload utility with progress tracking and resume support
 * Requirements: 2.1, 2.2, 2.3, 2.5
 */

import { getAccessToken } from '$lib/api/client';
import { CONFIG } from '$lib/config';

const DEFAULT_CHUNK_SIZE = CONFIG.upload.defaultChunkSize;

// API base URL
const API_BASE_URL = '/api/v1/stream';

/**
 * Upload progress callback
 */
export type UploadProgressCallback = (progress: UploadProgress) => void;

/**
 * Upload progress information
 */
export interface UploadProgress {
	uploadId: string;
	fileName: string;
	totalSize: number;
	uploadedSize: number;
	percentage: number;
	currentChunk: number;
	totalChunks: number;
	status: 'pending' | 'uploading' | 'complete' | 'error' | 'cancelled';
	error?: string;
}

/**
 * Upload options
 */
export interface UploadOptions {
	uploadId?: string;
	chunkSize?: number;
	onProgress?: UploadProgressCallback;
	signal?: AbortSignal;
}

/**
 * Upload response from the server
 */
interface UploadResponse {
	uploadId: string;
	chunkIndex: number;
	receivedChunks: number;
	totalChunks: number;
	complete: boolean;
	path?: string;
}

/**
 * Upload status response from the server
 */
interface UploadStatusResponse {
	uploadId: string;
	path: string;
	totalChunks: number;
	receivedChunks: number;
	missingChunks: number[];
	complete: boolean;
	createdAt: string;
	lastActivity: string;
}

/**
 * Generate a unique upload ID
 */
export function generateUploadId(): string {
	return `upload_${Date.now()}_${Math.random().toString(36).substring(2, 11)}`;
}

/**
 * Calculate SHA-256 checksum of a file
 */
export async function calculateChecksum(file: File): Promise<string> {
	const buffer = await file.arrayBuffer();
	const hashBuffer = await crypto.subtle.digest('SHA-256', buffer);
	const hashArray = Array.from(new Uint8Array(hashBuffer));
	return hashArray.map((b) => b.toString(16).padStart(2, '0')).join('');
}

/**
 * Calculate SHA-256 checksum of file chunks (for large files)
 */
export async function calculateChecksumStreaming(
	file: File,
	chunkSize: number = DEFAULT_CHUNK_SIZE
): Promise<string> {
	// For smaller files, use the simple method
	if (file.size <= chunkSize * 2) {
		return calculateChecksum(file);
	}

	// For larger files, we need to hash incrementally
	// Note: Web Crypto API doesn't support streaming, so we read the whole file
	// This is a limitation of the browser environment
	return calculateChecksum(file);
}

/**
 * Split a file into chunks
 */
export function* splitFileIntoChunks(
	file: File,
	chunkSize: number = DEFAULT_CHUNK_SIZE
): Generator<{ index: number; blob: Blob; isLast: boolean }> {
	const totalChunks = Math.ceil(file.size / chunkSize);

	for (let i = 0; i < totalChunks; i++) {
		const start = i * chunkSize;
		const end = Math.min(start + chunkSize, file.size);
		const blob = file.slice(start, end);

		yield {
			index: i,
			blob,
			isLast: i === totalChunks - 1
		};
	}
}

/**
 * Get the number of chunks for a file
 */
export function getChunkCount(fileSize: number, chunkSize: number = DEFAULT_CHUNK_SIZE): number {
	return Math.ceil(fileSize / chunkSize);
}

/**
 * Upload a single chunk
 */
async function uploadChunk(
	path: string,
	uploadId: string,
	chunkIndex: number,
	totalChunks: number,
	chunkSize: number,
	totalSize: number,
	chunkData: Blob,
	checksum?: string,
	signal?: AbortSignal
): Promise<UploadResponse> {
	const headers: Record<string, string> = {
		'X-Upload-ID': uploadId,
		'X-Chunk-Index': chunkIndex.toString(),
		'X-Total-Chunks': totalChunks.toString(),
		'X-Chunk-Size': chunkSize.toString(),
		'X-Total-Size': totalSize.toString(),
		'Content-Type': 'application/octet-stream'
	};

	// Add checksum on final chunk
	if (checksum) {
		headers['X-Checksum'] = `sha256:${checksum}`;
	}

	// Add auth token
	const token = getAccessToken();
	if (token) {
		headers['Authorization'] = `Bearer ${token}`;
	}

	const response = await fetch(`${API_BASE_URL}/upload/${path}`, {
		method: 'POST',
		headers,
		body: chunkData,
		signal
	});

	if (!response.ok) {
		const errorData = await response.json().catch(() => ({ error: 'Upload failed' }));
		throw new Error(errorData.error || `Upload failed with status ${response.status}`);
	}

	return response.json();
}

/**
 * Get upload status for resuming
 */
export async function getUploadStatus(uploadId: string): Promise<UploadStatusResponse | null> {
	const token = getAccessToken();
	const headers: Record<string, string> = {};

	if (token) {
		headers['Authorization'] = `Bearer ${token}`;
	}

	try {
		const response = await fetch(
			`${API_BASE_URL}/upload/status/?uploadId=${encodeURIComponent(uploadId)}`,
			{
				method: 'GET',
				headers
			}
		);

		if (response.status === 404) {
			return null;
		}

		if (!response.ok) {
			throw new Error('Failed to get upload status');
		}

		return response.json();
	} catch {
		return null;
	}
}

/**
 * Upload a file with chunking, progress tracking, and resume support
 */
export async function uploadFile(
	file: File,
	destinationPath: string,
	options: UploadOptions = {}
): Promise<{ success: boolean; path?: string; error?: string }> {
	const {
		uploadId = generateUploadId(),
		chunkSize = DEFAULT_CHUNK_SIZE,
		onProgress,
		signal
	} = options;

	const totalChunks = getChunkCount(file.size, chunkSize);

	// Initialize progress
	const progress: UploadProgress = {
		uploadId,
		fileName: file.name,
		totalSize: file.size,
		uploadedSize: 0,
		percentage: 0,
		currentChunk: 0,
		totalChunks,
		status: 'pending'
	};

	const reportProgress = () => {
		if (onProgress) {
			onProgress({ ...progress });
		}
	};

	try {
		// Calculate checksum before starting upload
		progress.status = 'uploading';
		reportProgress();

		const checksum = await calculateChecksumStreaming(file, chunkSize);

		// Upload chunks
		for (const chunk of splitFileIntoChunks(file, chunkSize)) {
			// Check for cancellation
			if (signal?.aborted) {
				progress.status = 'cancelled';
				reportProgress();
				return { success: false, error: 'Upload cancelled' };
			}

			progress.currentChunk = chunk.index;
			reportProgress();

			const response = await uploadChunk(
				destinationPath,
				uploadId,
				chunk.index,
				totalChunks,
				chunkSize,
				file.size,
				chunk.blob,
				chunk.isLast ? checksum : undefined,
				signal
			);

			// Update progress
			progress.uploadedSize = (chunk.index + 1) * chunkSize;
			if (progress.uploadedSize > file.size) {
				progress.uploadedSize = file.size;
			}
			progress.percentage = Math.round((progress.uploadedSize / file.size) * 100);
			reportProgress();

			if (response.complete) {
				progress.status = 'complete';
				progress.percentage = 100;
				progress.uploadedSize = file.size;
				reportProgress();
				return { success: true, path: response.path };
			}
		}

		// Should not reach here if upload completed successfully
		progress.status = 'complete';
		progress.percentage = 100;
		reportProgress();
		return { success: true, path: destinationPath };
	} catch (error) {
		progress.status = 'error';
		progress.error = error instanceof Error ? error.message : 'Upload failed';
		reportProgress();
		return { success: false, error: progress.error };
	}
}

/**
 * Resume an interrupted upload
 */
export async function resumeUpload(
	file: File,
	destinationPath: string,
	uploadId: string,
	options: UploadOptions = {}
): Promise<{ success: boolean; path?: string; error?: string }> {
	const { chunkSize = DEFAULT_CHUNK_SIZE, onProgress, signal } = options;

	// Get current upload status
	const status = await getUploadStatus(uploadId);

	if (!status) {
		// Session expired or not found, start fresh
		return uploadFile(file, destinationPath, { ...options, uploadId });
	}

	if (status.complete) {
		return { success: true, path: status.path };
	}

	const totalChunks = getChunkCount(file.size, chunkSize);
	const missingChunks = new Set(status.missingChunks);

	// Initialize progress
	const progress: UploadProgress = {
		uploadId,
		fileName: file.name,
		totalSize: file.size,
		uploadedSize: status.receivedChunks * chunkSize,
		percentage: Math.round((status.receivedChunks / totalChunks) * 100),
		currentChunk: status.receivedChunks,
		totalChunks,
		status: 'uploading'
	};

	const reportProgress = () => {
		if (onProgress) {
			onProgress({ ...progress });
		}
	};

	try {
		reportProgress();

		// Calculate checksum
		const checksum = await calculateChecksumStreaming(file, chunkSize);

		// Upload only missing chunks
		for (const chunk of splitFileIntoChunks(file, chunkSize)) {
			// Skip already uploaded chunks
			if (!missingChunks.has(chunk.index)) {
				continue;
			}

			// Check for cancellation
			if (signal?.aborted) {
				progress.status = 'cancelled';
				reportProgress();
				return { success: false, error: 'Upload cancelled' };
			}

			progress.currentChunk = chunk.index;
			reportProgress();

			const response = await uploadChunk(
				destinationPath,
				uploadId,
				chunk.index,
				totalChunks,
				chunkSize,
				file.size,
				chunk.blob,
				chunk.isLast ? checksum : undefined,
				signal
			);

			// Update progress
			missingChunks.delete(chunk.index);
			const uploadedChunks = totalChunks - missingChunks.size;
			progress.uploadedSize = uploadedChunks * chunkSize;
			if (progress.uploadedSize > file.size) {
				progress.uploadedSize = file.size;
			}
			progress.percentage = Math.round((progress.uploadedSize / file.size) * 100);
			reportProgress();

			if (response.complete) {
				progress.status = 'complete';
				progress.percentage = 100;
				progress.uploadedSize = file.size;
				reportProgress();
				return { success: true, path: response.path };
			}
		}

		progress.status = 'complete';
		progress.percentage = 100;
		reportProgress();
		return { success: true, path: destinationPath };
	} catch (error) {
		progress.status = 'error';
		progress.error = error instanceof Error ? error.message : 'Upload failed';
		reportProgress();
		return { success: false, error: progress.error };
	}
}

/**
 * Upload manager for handling multiple concurrent uploads
 */
export class UploadManager {
	private uploads: Map<
		string,
		{ file: File; path: string; progress: UploadProgress; abortController: AbortController }
	> = new Map();
	private onProgressCallback?: (uploads: UploadProgress[]) => void;

	constructor(onProgress?: (uploads: UploadProgress[]) => void) {
		this.onProgressCallback = onProgress;
	}

	/**
	 * Add a file to the upload queue and start uploading
	 */
	async addUpload(
		file: File,
		destinationPath: string,
		options: Omit<UploadOptions, 'signal' | 'onProgress'> = {}
	): Promise<string> {
		const uploadId = generateUploadId();
		const abortController = new AbortController();

		const progress: UploadProgress = {
			uploadId,
			fileName: file.name,
			totalSize: file.size,
			uploadedSize: 0,
			percentage: 0,
			currentChunk: 0,
			totalChunks: getChunkCount(file.size, options.chunkSize),
			status: 'pending'
		};

		this.uploads.set(uploadId, { file, path: destinationPath, progress, abortController });
		this.notifyProgress();

		// Start upload in background
		this.startUpload(uploadId, options);

		return uploadId;
	}

	/**
	 * Start or resume an upload
	 */
	private async startUpload(
		uploadId: string,
		options: Omit<UploadOptions, 'signal' | 'onProgress'> = {}
	): Promise<void> {
		const upload = this.uploads.get(uploadId);
		if (!upload) return;

		const result = await uploadFile(upload.file, upload.path, {
			...options,
			uploadId,
			signal: upload.abortController.signal,
			onProgress: (progress) => {
				const existing = this.uploads.get(uploadId);
				if (existing) {
					existing.progress = progress;
					this.notifyProgress();
				}
			}
		});

		if (!result.success && upload.progress.status !== 'cancelled') {
			upload.progress.status = 'error';
			upload.progress.error = result.error;
			this.notifyProgress();
		}
	}

	/**
	 * Cancel an upload
	 */
	cancelUpload(uploadId: string): void {
		const upload = this.uploads.get(uploadId);
		if (upload) {
			upload.abortController.abort();
			upload.progress.status = 'cancelled';
			this.notifyProgress();
		}
	}

	/**
	 * Remove an upload from the manager
	 */
	removeUpload(uploadId: string): void {
		const upload = this.uploads.get(uploadId);
		if (upload) {
			upload.abortController.abort();
			this.uploads.delete(uploadId);
			this.notifyProgress();
		}
	}

	/**
	 * Get all upload progress
	 */
	getUploads(): UploadProgress[] {
		return Array.from(this.uploads.values()).map((u) => ({ ...u.progress }));
	}

	/**
	 * Get a specific upload's progress
	 */
	getUpload(uploadId: string): UploadProgress | undefined {
		const upload = this.uploads.get(uploadId);
		return upload ? { ...upload.progress } : undefined;
	}

	/**
	 * Notify progress callback
	 */
	private notifyProgress(): void {
		if (this.onProgressCallback) {
			this.onProgressCallback(this.getUploads());
		}
	}

	/**
	 * Clear completed/failed/cancelled uploads
	 */
	clearFinished(): void {
		for (const [id, upload] of this.uploads) {
			if (['complete', 'error', 'cancelled'].includes(upload.progress.status)) {
				this.uploads.delete(id);
			}
		}
		this.notifyProgress();
	}
}
