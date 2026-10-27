import { normalizeAccentColor } from '#lib/utils/appearance.js';

const THEME_COLOR_PROPERTIES = [
	'--color-accent',
	'--color-accent-base',
	'--color-accent-icon',
	'--color-accent-hover',
	'--color-accent-muted',
	'--color-accent-solid',
	'--color-accent-solid-hover',
	'--color-border-focus',
	'--color-folder',
	'--color-selection',
	'--color-selection-hover'
];

export function parseHexColor(color: string): [number, number, number] {
	const hex = color.slice(1);
	return [
		Number.parseInt(hex.slice(0, 2), 16),
		Number.parseInt(hex.slice(2, 4), 16),
		Number.parseInt(hex.slice(4, 6), 16)
	];
}

export function toHexColor([red, green, blue]: [number, number, number]): string {
	return `#${[red, green, blue]
		.map((value) => Math.round(value).toString(16).padStart(2, '0'))
		.join('')}`;
}

export function mixColor(color: string, target: string, amount: number): string {
	const sourceRgb = parseHexColor(color);
	const targetRgb = parseHexColor(target);

	return toHexColor([
		sourceRgb[0] + (targetRgb[0] - sourceRgb[0]) * amount,
		sourceRgb[1] + (targetRgb[1] - sourceRgb[1]) * amount,
		sourceRgb[2] + (targetRgb[2] - sourceRgb[2]) * amount
	]);
}

function linearChannel(channel: number): number {
	const value = channel / 255;
	return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
}

export function luminance([red, green, blue]: [number, number, number]): number {
	return 0.2126 * linearChannel(red) + 0.7152 * linearChannel(green) + 0.0722 * linearChannel(blue);
}

/** The accent, darkened until white text on it meets the 4.5:1 contrast of WCAG AA. */
export function solidAccentColor(accent: string): string {
	for (let amount = 0; amount <= 0.9; amount += 0.05) {
		const candidate = mixColor(accent, '#000000', amount);
		if (1.05 / (luminance(parseHexColor(candidate)) + 0.05) >= 4.5) return candidate;
	}
	return '#000000';
}

export function applyAccentColor(
	accentColor: string | null,
	textColor?: string,
	iconColor?: string
): void {
	if (typeof document === 'undefined') return;

	const color = normalizeAccentColor(accentColor);
	const rootStyle = document.documentElement.style;

	if (!color) {
		for (const property of THEME_COLOR_PROPERTIES) {
			rootStyle.removeProperty(property);
		}
		return;
	}

	const ink = normalizeAccentColor(textColor ?? null) ?? color;
	rootStyle.setProperty('--color-accent-base', color);
	rootStyle.setProperty('--color-accent-icon', normalizeAccentColor(iconColor ?? null) ?? ink);
	rootStyle.setProperty('--color-accent', ink);
	rootStyle.setProperty('--color-border-focus', ink);
	rootStyle.setProperty('--color-folder', ink);
	rootStyle.setProperty('--color-accent-hover', mixColor(color, '#000000', 0.32));
	const solid = solidAccentColor(color);
	rootStyle.setProperty('--color-accent-solid', solid);
	rootStyle.setProperty('--color-accent-solid-hover', mixColor(solid, '#000000', 0.15));
	rootStyle.setProperty('--color-accent-muted', mixColor(color, '#1e1e1e', 0.55));
	rootStyle.setProperty('--color-selection', mixColor(color, '#1e1e1e', 0.55));
	rootStyle.setProperty(
		'--color-selection-hover',
		solidAccentColor(mixColor(color, '#000000', 0.32))
	);
}
