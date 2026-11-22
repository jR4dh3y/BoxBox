import { SvelteSet } from 'svelte/reactivity';
import type { FileInfo } from '#lib/api/files.js';
import {
	BAND_THRESHOLD,
	edgeScrollStep,
	itemAt,
	itemsInBand,
	type BandLayout,
	type Point
} from './band.js';

export interface RubberBandHost {
	/** Where the items start below the scrolled content's top, such as under a header. */
	itemsTop: number;
	layout(): BandLayout;
	items(): readonly FileInfo[];
	selected(): ReadonlySet<string>;
	select(paths: Set<string>): void;
	/** Keyboard navigation continues from the view the band started in. */
	focus(): void;
}

/** The rectangle as drawn, in the scrolled content's coordinates. */
export interface BandRect {
	left: number;
	top: number;
	width: number;
	height: number;
}

/**
 * A drag that starts on empty space selects what it touches, Ctrl or Shift adding to the
 * selection; held past the top or bottom edge, it scrolls. A click on empty space clears the
 * selection. Use as an action on the element that scrolls.
 */
export class RubberBand {
	rect = $state.raw<BandRect | null>(null);
	private readonly host: () => RubberBandHost;

	constructor(host: () => RubberBandHost) {
		this.host = host;
	}

	attach = (viewport: HTMLElement) => {
		const press = (event: PointerEvent) => this.press(event, viewport);
		viewport.addEventListener('pointerdown', press);
		return { destroy: () => viewport.removeEventListener('pointerdown', press) };
	};

	private content(viewport: HTMLElement, clientX: number, clientY: number): Point {
		const bounds = viewport.getBoundingClientRect();
		return {
			x: clientX - bounds.left + viewport.scrollLeft,
			y: clientY - bounds.top + viewport.scrollTop - this.host().itemsTop
		};
	}

	private press(event: PointerEvent, viewport: HTMLElement): void {
		if (event.button !== 0 || event.pointerType === 'touch') return;
		const target = event.target instanceof Element ? event.target : null;
		// Items, headers and controls keep their own presses.
		if (target?.closest('[data-file-row], [data-file-item], [data-file-header], input, button'))
			return;
		const host = this.host();
		const origin = this.content(viewport, event.clientX, event.clientY);
		if (origin.y < 0 || itemAt(host.layout(), origin, host.items().length) !== null) return;
		const bounds = viewport.getBoundingClientRect();
		// The scrollbar is not empty space.
		if (event.clientX >= bounds.left + viewport.clientWidth) return;

		const additive = event.ctrlKey || event.metaKey || event.shiftKey;
		const base = [...host.selected()];
		let pointer = { x: event.clientX, y: event.clientY };
		let dragging = false;
		let frame = 0;

		const select = () => {
			const corner = this.content(viewport, pointer.x, pointer.y);
			const current = this.host();
			const items = current.items();
			const touched = itemsInBand(current.layout(), origin, corner, items.length);
			const paths = new SvelteSet(additive ? base : []);
			for (const index of touched) {
				const item = items[index];
				if (item) paths.add(item.path);
			}
			current.select(paths);
			this.rect = {
				left: Math.min(origin.x, corner.x),
				top: Math.min(origin.y, corner.y) + current.itemsTop,
				width: Math.abs(corner.x - origin.x),
				height: Math.abs(corner.y - origin.y)
			};
		};
		const scroll = () => {
			const edges = viewport.getBoundingClientRect();
			const step = edgeScrollStep(pointer.y, edges.top + this.host().itemsTop, edges.bottom);
			if (step !== 0) {
				const before = viewport.scrollTop;
				viewport.scrollTop = before + step;
				if (viewport.scrollTop !== before) select();
			}
			frame = requestAnimationFrame(scroll);
		};
		const move = (moved: PointerEvent) => {
			pointer = { x: moved.clientX, y: moved.clientY };
			if (!dragging) {
				const at = this.content(viewport, pointer.x, pointer.y);
				if (Math.max(Math.abs(at.x - origin.x), Math.abs(at.y - origin.y)) < BAND_THRESHOLD) return;
				dragging = true;
				frame = requestAnimationFrame(scroll);
			}
			select();
		};
		const release = () => {
			window.removeEventListener('pointermove', move);
			window.removeEventListener('pointerup', release);
			window.removeEventListener('pointercancel', release);
			cancelAnimationFrame(frame);
			this.rect = null;
			if (!dragging && !additive && this.host().selected().size > 0)
				this.host().select(new SvelteSet());
		};
		window.addEventListener('pointermove', move);
		window.addEventListener('pointerup', release);
		window.addEventListener('pointercancel', release);
		// A drag here selects; it does not select text.
		event.preventDefault();
		host.focus();
	}
}
