import { isRecord } from '#lib/api/parse.js';

/**
 * Zoom, as in Nautilus, Dolphin and the desktop app: Ctrl+plus, Ctrl+minus and Ctrl+wheel step
 * the size of the view, Ctrl+0 resets it. The list and the grid keep their own step, for every
 * folder. Step 1 is the original size and step 0 the old compact one.
 */

export interface ListMetrics {
	rowHeight: number;
	icon: number;
}

export interface GridMetrics {
	tileWidth: number;
	rowHeight: number;
	gap: number;
	padding: number;
	previewHeight: number;
	icon: number;
	/** The thumbnail asked for; steps 0 and 1 keep the sizes servers already cached. */
	thumbnail: { width: number; height: number };
}

const LIST_DEFAULT: ListMetrics = { rowHeight: 32, icon: 16 };
export const LIST_STEPS: readonly ListMetrics[] = [
	{ rowHeight: 28, icon: 16 },
	LIST_DEFAULT,
	{ rowHeight: 40, icon: 24 },
	{ rowHeight: 48, icon: 32 }
];

const GRID_DEFAULT: GridMetrics = {
	tileWidth: 112,
	rowHeight: 164,
	gap: 12,
	padding: 12,
	previewHeight: 80,
	icon: 30,
	thumbnail: { width: 224, height: 144 }
};
export const GRID_STEPS: readonly GridMetrics[] = [
	{
		tileWidth: 96,
		rowHeight: 136,
		gap: 8,
		padding: 8,
		previewHeight: 56,
		icon: 24,
		thumbnail: { width: 192, height: 96 }
	},
	GRID_DEFAULT,
	{
		tileWidth: 144,
		rowHeight: 196,
		gap: 12,
		padding: 12,
		previewHeight: 112,
		icon: 44,
		thumbnail: { width: 288, height: 200 }
	},
	{
		tileWidth: 192,
		rowHeight: 244,
		gap: 12,
		padding: 12,
		previewHeight: 160,
		icon: 64,
		thumbnail: { width: 384, height: 288 }
	},
	{
		tileWidth: 256,
		rowHeight: 308,
		gap: 12,
		padding: 12,
		previewHeight: 224,
		icon: 88,
		thumbnail: { width: 512, height: 400 }
	}
];

/** The step Ctrl+0 returns to. */
export const DEFAULT_ZOOM = 1;

function clampStep(step: number, count: number): number {
	return Math.min(count - 1, Math.max(0, Math.trunc(step)));
}

export function listMetrics(step: number): ListMetrics {
	return LIST_STEPS[clampStep(step, LIST_STEPS.length)] ?? LIST_DEFAULT;
}

export function gridMetrics(step: number): GridMetrics {
	return GRID_STEPS[clampStep(step, GRID_STEPS.length)] ?? GRID_DEFAULT;
}

/**
 * A saved zoom step. Settings from before zoom kept a compact flag instead; compact rows are
 * step 0 of both views.
 */
export function savedZoom(saved: unknown, key: 'listZoom' | 'gridZoom'): number {
	if (!isRecord(saved)) return DEFAULT_ZOOM;
	const step = saved[key];
	if (typeof step === 'number' && Number.isInteger(step) && step >= 0) return step;
	return saved.compactMode === true ? 0 : DEFAULT_ZOOM;
}

export type ZoomChange = 'in' | 'out' | 'reset';

/** The step after `change`, kept within the view's steps. */
export function zoomStep(current: number, change: ZoomChange, count: number): number {
	if (change === 'reset') return DEFAULT_ZOOM;
	return clampStep(current + (change === 'in' ? 1 : -1), count);
}

/** Touchpad pixels per zoom step. */
const PIXELS_PER_STEP = 40;
/** Browsers report a mouse notch as about 100 pixels; a touchpad sends small deltas. */
const NOTCH_PIXELS = 50;

/** Ctrl+wheel: a mouse notch is a step; a touchpad steps every few pixels. */
export class WheelZoom {
	private pixels = 0;

	/** Steps for a wheel event: positive zooms in. */
	steps(deltaY: number, deltaMode: number): number {
		if (deltaMode !== 0 || Math.abs(deltaY) >= NOTCH_PIXELS) {
			this.pixels = 0;
			return -Math.sign(deltaY);
		}
		this.pixels -= deltaY;
		const steps = Math.trunc(this.pixels / PIXELS_PER_STEP);
		this.pixels -= steps * PIXELS_PER_STEP;
		return steps;
	}
}
