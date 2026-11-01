import type { DesktopVolume } from '#lib/api/desktopVolumes.js';
import type { DriveStats } from '#lib/api/files.js';
/** Mounted volumes appear alongside ordinary local drives in This PC. */
export function mountedVolumeDrives(volumes: readonly DesktopVolume[]): DriveStats[] {
	return volumes.flatMap((volume) => {
		if (!volume.mounted || !volume.browsePath) return [];
		const totalBytes = volume.totalBytes ?? 0;
		const freeBytes = volume.freeBytes ?? 0;
		const usedBytes = Math.max(0, totalBytes - freeBytes);
		return [
			{
				name: volume.browsePath,
				label: volume.name,
				path: volume.browsePath,
				device: volume.device,
				totalBytes,
				freeBytes,
				usedBytes,
				usedPct: totalBytes ? (usedBytes / totalBytes) * 100 : 0,
				readOnly: volume.readOnly
			}
		];
	});
}
