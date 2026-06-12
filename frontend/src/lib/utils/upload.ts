/**
 * Chunked upload utility with progress tracking and resume support
 * Requirements: 2.1, 2.2, 2.3, 2.5
 */

import { getAccessToken } from '$lib/api/client';
import { CONFIG } from '$lib/config';

const DEFAULT_CHUNK_SIZE = CONFIG.upload.defaultChunkSize;
const MAX_CHECKSUM_BYTES = DEFAULT_CHUNK_SIZE * 2;

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
): Promise<string | undefined> {
	const checksumLimit = Math.max(chunkSize * 2, MAX_CHECKSUM_BYTES);
	if (file.size > checksumLimit) {
		return undefined;
	}

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

function calculateUploadedSize(
	uploadedChunks: number,
	fileSize: number,
	chunkSize: number
): number {
	return Math.min(uploadedChunks * chunkSize, fileSize);
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
	signal?: AbortSignal,
	onUploadProgress?: (loaded: number) => void
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

	return new Promise((resolve, reject) => {
		const xhr = new XMLHttpRequest();
		const abortHandler = () => {
			xhr.abort();
			reject(new Error('Upload cancelled'));
		};

		xhr.open('POST', `${API_BASE_URL}/upload/${path}`);

		for (const [key, value] of Object.entries(headers)) {
			xhr.setRequestHeader(key, value);
		}

		xhr.upload.onprogress = (event) => {
			if (event.lengthComputable) {
				onUploadProgress?.(event.loaded);
			}
		};

		xhr.onload = () => {
			signal?.removeEventListener('abort', abortHandler);

			if (xhr.status < 200 || xhr.status >= 300) {
				let message = `Upload failed with status ${xhr.status}`;
				try {
					const errorData = JSON.parse(xhr.responseText) as { error?: string };
					message = errorData.error || message;
				} catch {
					// Keep the HTTP status message when the server response is not JSON.
				}
				reject(new Error(message));
				return;
			}

			try {
				resolve(JSON.parse(xhr.responseText) as UploadResponse);
			} catch {
				reject(new Error('Invalid upload response'));
			}
		};

		xhr.onerror = () => {
			signal?.removeEventListener('abort', abortHandler);
			reject(new Error('Upload failed'));
		};
		xhr.onabort = () => {
			signal?.removeEventListener('abort', abortHandler);
			reject(new Error('Upload cancelled'));
		};

		if (signal?.aborted) {
			abortHandler();
			return;
		}

		signal?.addEventListener('abort', abortHandler, { once: true });
		xhr.send(chunkData);
	});
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
				signal,
				(loaded) => {
					progress.uploadedSize = Math.min(chunk.index * chunkSize + loaded, file.size);
					progress.percentage = Math.round((progress.uploadedSize / file.size) * 100);
					reportProgress();
				}
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
				signal,
				(loaded) => {
					const uploadedChunksBeforeCurrent = totalChunks - missingChunks.size;
					const baseUploadedSize = calculateUploadedSize(
						uploadedChunksBeforeCurrent,
						file.size,
						chunkSize
					);
					progress.uploadedSize = Math.min(baseUploadedSize + loaded, file.size);
					progress.percentage = Math.round((progress.uploadedSize / file.size) * 100);
					reportProgress();
				}
			);

			// Update progress
			missingChunks.delete(chunk.index);
			const uploadedChunks = totalChunks - missingChunks.size;
			progress.uploadedSize = calculateUploadedSize(uploadedChunks, file.size, chunkSize);
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
