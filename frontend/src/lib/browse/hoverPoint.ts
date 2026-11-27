/**
 * What rows, tiles and sidebar drives report to the hover cards. The cards' own code loads on the
 * first rest (`hoverCards.svelte.ts`), so this stays small.
 */
import type { FileInfo } from '#lib/api/files.js';

export interface HoverPoint {
	x: number;
	y: number;
	touch: boolean;
}

export function hoverPoint(event: PointerEvent): HoverPoint {
	return { x: event.clientX, y: event.clientY, touch: event.pointerType === 'touch' };
}

/** What a row or tile tells the card. */
export interface HoverTarget {
	rest(item: FileInfo, at: HoverPoint): void;
	hide(): void;
}
