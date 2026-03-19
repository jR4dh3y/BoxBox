/**
 * API module exports
 */

// Client utilities
export {
	api,
	apiRequest,
	ApiRequestError,
	getAccessToken,
	getRefreshToken,
	setTokens,
	clearTokens,
	isAuthenticated,
	type ApiError,
	type TokenPair,
	type RequestOptions
} from './client';

// Auth API
export { login, refresh, logout, type LoginRequest, type LoginResponse } from './auth';

// Files API
export {
	listRoots,
	getPath,
	listDirectory,
	getFileInfo,
	createDirectory,
	rename,
	deleteFile,
	search,
	type FileInfo,
	type FileList,
	type MountPoint,
	type RootsResponse,
	type ListOptions,
	type SearchResponse
} from './files';

// Jobs API
export {
	listJobs,
	getJob,
	createJob,
	createCopyJob,
	createMoveJob,
	createDeleteJob,
	cancelJob,
	isJobTerminal,
	isJobActive,
	type Job,
	type JobType,
	type JobState,
	type JobListResponse,
	type CreateJobRequest
} from './jobs';

// System API
export { getSystemDrives, type SystemDrive, type SystemDrivesResponse } from './system';
