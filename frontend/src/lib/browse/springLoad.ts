/**
 * Spring-loaded folders, as GTK's path bar, Thunar and the desktop app have them: a drag that
 * rests on a folder, a path crumb or a place for a moment opens it, so a drop can go deeper.
 */

/** How long a drag rests on a target before it opens. */
export const SPRING_OPEN_MS = 700;

export class SpringLoad {
	private key: string | null = null;
	private timer: ReturnType<typeof setTimeout> | undefined;
	private readonly delayMs: number;

	constructor(delayMs = SPRING_OPEN_MS) {
		this.delayMs = delayMs;
	}

	/** The drag is over `key`; `open` runs if it is still there after the delay. */
	rest(key: string, open: () => void): void {
		if (this.key === key) return;
		this.cancel();
		this.key = key;
		this.timer = setTimeout(() => {
			this.key = null;
			open();
		}, this.delayMs);
	}

	/** The drag left, dropped or ended. */
	cancel(): void {
		clearTimeout(this.timer);
		this.key = null;
	}
}

/** Near the top or bottom of a list, a drag scrolls it; deeper in, faster. */
export const DRAG_SCROLL_EDGE = 40;
const DRAG_MAX_SCROLL_STEP = 16;

/** How far a drag at `y` scrolls a view spanning `top` to `bottom`, per dragover. */
export function dragScrollStep(y: number, top: number, bottom: number): number {
	if (y < top + DRAG_SCROLL_EDGE)
		return -Math.ceil(((top + DRAG_SCROLL_EDGE - y) / DRAG_SCROLL_EDGE) * DRAG_MAX_SCROLL_STEP);
	if (y > bottom - DRAG_SCROLL_EDGE)
		return Math.ceil(((y - (bottom - DRAG_SCROLL_EDGE)) / DRAG_SCROLL_EDGE) * DRAG_MAX_SCROLL_STEP);
	return 0;
}
