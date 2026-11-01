export type FileOpenBehavior = 'single-click' | 'double-click';

/** Preserve the legacy preview preference when loading saved settings. */
export function normalizeFileOpenBehavior(
	value: unknown,
	legacyPreviewOnSingleClick?: unknown
): FileOpenBehavior {
	if (value === 'single-click' || value === 'double-click') return value;
	return legacyPreviewOnSingleClick === true ? 'single-click' : 'double-click';
}
