import type { Measure } from '#lib/browse/tileLabel.js';

/**
 * Measures text in one CSS font with a canvas, a character at a time. Kerning is lost,
 * which costs a pixel or two; widths are cached, so a full grid measures in microseconds.
 */
export function canvasMeasure(font: string): Measure {
	const context = document.createElement('canvas').getContext('2d');
	if (context) context.font = font;
	const widths = new Map<string, number>();
	const charWidth = (character: string): number => {
		let width = widths.get(character);
		if (width === undefined) {
			width = context ? context.measureText(character).width : character.length * 7;
			widths.set(character, width);
		}
		return width;
	};
	return (text) => {
		const starts: [number, number][] = [];
		let x = 0;
		let index = 0;
		for (const character of text) {
			starts.push([index, x]);
			x += charWidth(character);
			index += character.length;
		}
		return { starts, width: x };
	};
}
