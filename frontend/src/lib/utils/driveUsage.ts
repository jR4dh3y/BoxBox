export type DriveUsageLevel = 'normal' | 'warning' | 'danger';

export function driveUsageLevel(usedPct: number): DriveUsageLevel {
	if (usedPct >= 90) return 'danger';
	if (usedPct >= 75) return 'warning';
	return 'normal';
}

/** Bar colour for each level; fixed so usage stays readable under any accent. */
export const DRIVE_USAGE_BAR = {
	normal: 'info',
	warning: 'warning',
	danger: 'danger'
} as const satisfies Record<DriveUsageLevel, 'info' | 'warning' | 'danger'>;

export const DRIVE_USAGE_TEXT: Record<DriveUsageLevel, string> = {
	normal: 'text-text-muted',
	warning: 'text-warning',
	danger: 'text-danger'
};
