/**
 * Formatting utilities for file sizes and dates
 * Requirements: 1.1
 */

/**
 * File size units for formatting
 */
const SIZE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'] as const;

/**
 * Format a file size in bytes to a human-readable string
 * @param bytes - The size in bytes
 * @param decimals - Number of decimal places (default: 2)
 * @returns Formatted string like "1.5 MB"
 */
export function formatFileSize(bytes: number, decimals: number = 2): string {
	if (bytes === 0) return '0 B';
	if (bytes < 0) return 'Invalid size';
	if (!Number.isFinite(bytes)) return 'Invalid size';

	const k = 1024;
	const dm = Math.max(0, decimals);
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	const unitIndex = Math.min(i, SIZE_UNITS.length - 1);

	const value = bytes / Math.pow(k, unitIndex);

	// Remove trailing zeros for cleaner display
	const formatted = value.toFixed(dm);
	const trimmed = parseFloat(formatted).toString();

	return `${trimmed} ${SIZE_UNITS[unitIndex]}`;
}

/**
 * Format a file size with consistent decimal places (no trimming)
 * @param bytes - The size in bytes
 * @param decimals - Number of decimal places (default: 2)
 * @returns Formatted string like "1.50 MB"
 */
export function formatFileSizeFixed(bytes: number, decimals: number = 2): string {
	if (bytes === 0) return '0 B';
	if (bytes < 0) return 'Invalid size';
	if (!Number.isFinite(bytes)) return 'Invalid size';

	const k = 1024;
	const dm = Math.max(0, decimals);
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	const unitIndex = Math.min(i, SIZE_UNITS.length - 1);

	const value = bytes / Math.pow(k, unitIndex);

	return `${value.toFixed(dm)} ${SIZE_UNITS[unitIndex]}`;
}

/**
 * Parse a human-readable file size string back to bytes
 * @param sizeStr - String like "1.5 MB" or "1.5MB"
 * @returns Size in bytes, or NaN if invalid
 */
export function parseFileSize(sizeStr: string): number {
	const match = sizeStr.trim().match(/^([\d.]+)\s*([A-Za-z]+)$/);
	if (!match) return NaN;

	const value = parseFloat(match[1]);
	const unit = match[2].toUpperCase();

	const unitIndex = SIZE_UNITS.indexOf(unit as (typeof SIZE_UNITS)[number]);
	if (unitIndex === -1) return NaN;

	return value * Math.pow(1024, unitIndex);
}

/**
 * Date formatting options
 */
export interface DateFormatOptions {
	includeTime?: boolean;
	includeSeconds?: boolean;
	relative?: boolean;
	locale?: string;
}

/**
 * Format a date to a human-readable string
 * @param date - Date object, ISO string, or timestamp
 * @param options - Formatting options
 * @returns Formatted date string
 */
export function formatDate(date: Date | string | number, options: DateFormatOptions = {}): string {
	const {
		includeTime = true,
		includeSeconds = false,
		relative = false,
		locale = 'en-US'
	} = options;

	const dateObj = date instanceof Date ? date : new Date(date);

	if (isNaN(dateObj.getTime())) {
		return 'Invalid date';
	}

	// Relative time formatting
	if (relative) {
		return formatRelativeTime(dateObj);
	}

	// Absolute time formatting
	const dateOptions: Intl.DateTimeFormatOptions = {
		year: 'numeric',
		month: 'short',
		day: 'numeric'
	};

	if (includeTime) {
		dateOptions.hour = '2-digit';
		dateOptions.minute = '2-digit';
		if (includeSeconds) {
			dateOptions.second = '2-digit';
		}
	}

	return dateObj.toLocaleString(locale, dateOptions);
}

/**
 * Format a date as relative time (e.g., "2 hours ago", "in 3 days")
 * @param date - Date object, ISO string, or timestamp
 * @returns Relative time string
 */
export function formatRelativeTime(date: Date | string | number): string {
	const dateObj = date instanceof Date ? date : new Date(date);

	if (isNaN(dateObj.getTime())) {
		return 'Invalid date';
	}

	const now = new Date();
	const diffMs = dateObj.getTime() - now.getTime();
	const diffSec = Math.round(diffMs / 1000);
	const diffMin = Math.round(diffSec / 60);
	const diffHour = Math.round(diffMin / 60);
	const diffDay = Math.round(diffHour / 24);
	const diffWeek = Math.round(diffDay / 7);
	const diffMonth = Math.round(diffDay / 30);
	const diffYear = Math.round(diffDay / 365);

	// Past times
	if (diffMs < 0) {
		const absSec = Math.abs(diffSec);
		const absMin = Math.abs(diffMin);
		const absHour = Math.abs(diffHour);
		const absDay = Math.abs(diffDay);
		const absWeek = Math.abs(diffWeek);
		const absMonth = Math.abs(diffMonth);
		const absYear = Math.abs(diffYear);

		if (absSec < 60) return 'just now';
		if (absMin < 60) return `${absMin} minute${absMin === 1 ? '' : 's'} ago`;
		if (absHour < 24) return `${absHour} hour${absHour === 1 ? '' : 's'} ago`;
		if (absDay < 7) return `${absDay} day${absDay === 1 ? '' : 's'} ago`;
		if (absWeek < 4) return `${absWeek} week${absWeek === 1 ? '' : 's'} ago`;
		if (absMonth < 12) return `${absMonth} month${absMonth === 1 ? '' : 's'} ago`;
		return `${absYear} year${absYear === 1 ? '' : 's'} ago`;
	}

	// Future times
	if (diffSec < 60) return 'in a moment';
	if (diffMin < 60) return `in ${diffMin} minute${diffMin === 1 ? '' : 's'}`;
	if (diffHour < 24) return `in ${diffHour} hour${diffHour === 1 ? '' : 's'}`;
	if (diffDay < 7) return `in ${diffDay} day${diffDay === 1 ? '' : 's'}`;
	if (diffWeek < 4) return `in ${diffWeek} week${diffWeek === 1 ? '' : 's'}`;
	if (diffMonth < 12) return `in ${diffMonth} month${diffMonth === 1 ? '' : 's'}`;
	return `in ${diffYear} year${diffYear === 1 ? '' : 's'}`;
}

/**
 * Format a date for display in file listings (compact format)
 * @param date - Date object, ISO string, or timestamp
 * @returns Compact date string
 */
export function formatFileDate(date: Date | string | number): string {
	const dateObj = date instanceof Date ? date : new Date(date);

	if (isNaN(dateObj.getTime())) {
		return '-';
	}

	const now = new Date();
	const isToday = dateObj.toDateString() === now.toDateString();
	const isThisYear = dateObj.getFullYear() === now.getFullYear();

	if (isToday) {
		return dateObj.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
	}

	if (isThisYear) {
		return dateObj.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
	}

	return dateObj.toLocaleDateString('en-US', { year: 'numeric', month: 'short', day: 'numeric' });
}

/**
 * Format a duration in milliseconds to a human-readable string
 * @param ms - Duration in milliseconds
 * @returns Formatted duration string like "2h 30m" or "45s"
 */
export function formatDuration(ms: number): string {
	if (ms < 0) return 'Invalid duration';
	if (ms === 0) return '0s';

	const seconds = Math.floor(ms / 1000);
	const minutes = Math.floor(seconds / 60);
	const hours = Math.floor(minutes / 60);
	const days = Math.floor(hours / 24);

	const parts: string[] = [];

	if (days > 0) parts.push(`${days}d`);
	if (hours % 24 > 0) parts.push(`${hours % 24}h`);
	if (minutes % 60 > 0) parts.push(`${minutes % 60}m`);
	if (seconds % 60 > 0 && days === 0) parts.push(`${seconds % 60}s`);

	return parts.length > 0 ? parts.join(' ') : '0s';
}

/**
 * Format a transfer speed in bytes per second
 * @param bytesPerSecond - Speed in bytes per second
 * @returns Formatted speed string like "1.5 MB/s"
 */
export function formatTransferSpeed(bytesPerSecond: number): string {
	if (bytesPerSecond < 0) return 'Invalid speed';
	if (bytesPerSecond === 0) return '0 B/s';

	return `${formatFileSize(bytesPerSecond)}/s`;
}

/**
 * Format a percentage value
 * @param value - Value between 0 and 1, or 0 and 100
 * @param decimals - Number of decimal places (default: 0)
 * @param assumeDecimal - If true, assumes value is 0-1 range (default: auto-detect)
 * @returns Formatted percentage string like "75%"
 */
export function formatPercentage(
	value: number,
	decimals: number = 0,
	assumeDecimal?: boolean
): string {
	if (!Number.isFinite(value)) return 'Invalid';

	// Auto-detect if value is in 0-1 or 0-100 range
	const isDecimal = assumeDecimal ?? (value >= 0 && value <= 1);
	const percentage = isDecimal ? value * 100 : value;

	return `${percentage.toFixed(decimals)}%`;
}

/**
 * Truncate a filename while preserving the extension
 * @param filename - The filename to truncate
 * @param maxLength - Maximum length (default: 30)
 * @returns Truncated filename with extension preserved
 */
export function truncateFilename(filename: string, maxLength: number = 30): string {
	if (filename.length <= maxLength) return filename;

	const lastDot = filename.lastIndexOf('.');
	const hasExtension = lastDot > 0 && lastDot < filename.length - 1;

	if (!hasExtension) {
		return filename.slice(0, maxLength - 3) + '...';
	}

	const extension = filename.slice(lastDot);
	const name = filename.slice(0, lastDot);
	const availableLength = maxLength - extension.length - 3; // 3 for "..."

	if (availableLength <= 0) {
		return filename.slice(0, maxLength - 3) + '...';
	}

	return name.slice(0, availableLength) + '...' + extension;
}

// Note: getFileTypeDescription has been moved to $lib/utils/fileTypes.ts
// Import it from there: import { getFileTypeDescription } from '$lib/utils/fileTypes';
export { getFileTypeDescription } from '$lib/utils/fileTypes';

import { CONFIG } from '$lib/config';

/**
 * Map a system mount point (as returned by df/system drives API) to a browsable path in the app.
 * This handles the translation from container-internal paths to the app's mount point names.
 *
 * Examples:
 * - '/media/devmon' -> 'drives'
 * - '/media/devmon/usb-drive' -> 'drives/usb-drive'
 * - '/home/user' -> 'home'
 * - '/host_root' -> 'root'
 * - '/host_root/mnt/rclone' -> 'root/mnt/rclone'
 *
 * @param mountPoint - The system mount point path
 * @returns The browsable path that can be used with handleNavigate
 */
export function mapSystemMountToBrowsePath(mountPoint: string): string {
	// Find matching mount point mapping
	for (const mapping of CONFIG.paths.mountPointMappings) {
		if (mountPoint === mapping.systemPath) {
			// Exact match
			return mapping.browsePath;
		} else if (mountPoint.startsWith(mapping.systemPath + '/')) {
			// Subpath match - replace prefix with browse path
			const subPath = mountPoint.slice(mapping.systemPath.length);
			return mapping.browsePath + subPath;
		}
	}

	// Fallback for root filesystem
	if (mountPoint === '/') {
		return CONFIG.paths.hostRootMount;
	}

	// Unknown mount - log warning and try using it as-is
	console.warn('Unknown system mount point:', mountPoint);
	return mountPoint.startsWith('/') ? mountPoint.slice(1) : mountPoint;
}
