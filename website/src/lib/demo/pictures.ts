// Sample photos for the preview: a small SVG landscape drawn from the file
// name, so the page ships no stock images and every name gets its own picture.

import type { ImageFile } from './fs';

const skies: [string, string][] = [
  ['#ff9a6b', '#a85c8a'],
  ['#3ddc84', '#1c6b52'],
  ['#8b6cff', '#3b2a7a'],
  ['#3aa0ff', '#1c3f8a'],
  ['#ffb347', '#b3562e'],
  ['#ff4fa3', '#6b2a6e'],
  ['#35d6e6', '#1a5f8a'],
  ['#7ee0c0', '#2b6f74']
];

function hash(text: string): number {
  let value = 2166136261;
  for (let index = 0; index < text.length; index += 1) {
    value = Math.imul(value ^ text.charCodeAt(index), 16777619);
  }
  return value >>> 0;
}

function random(seed: number): () => number {
  let state = seed;
  return () => {
    state = (state + 0x6d2b79f5) | 0;
    let mixed = Math.imul(state ^ (state >>> 15), 1 | state);
    mixed = (mixed + Math.imul(mixed ^ (mixed >>> 7), 61 | mixed)) ^ mixed;
    return ((mixed ^ (mixed >>> 14)) >>> 0) / 4294967296;
  };
}

function ridge(next: () => number, base: number, height: number): string {
  const steps = 6;
  const points = Array.from({ length: steps + 1 }, (_, step) => {
    const y = base - next() * height;
    return `${Math.round((step / steps) * 320)},${Math.round(y)}`;
  });
  return `0,200 ${points.join(' ')} 320,200`;
}

export function picture(name: string): ImageFile {
  const seed = hash(name);
  const next = random(seed);
  // A sunset is the one name that promises a colour.
  const [top, bottom] = skies[name.startsWith('sunset') ? 0 : seed % skies.length] ?? ['#ff9a6b', '#a85c8a'];
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 200">` +
    `<defs><linearGradient id="s" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${top}"/><stop offset="1" stop-color="${bottom}"/></linearGradient></defs>` +
    `<rect width="320" height="200" fill="url(#s)"/>` +
    `<polygon fill="#1b1730" points="${ridge(next, 120, 60)}"/>` +
    `<polygon fill="#0b0a14" points="${ridge(next, 165, 50)}"/>` +
    `</svg>`;
  return {
    kind: 'image',
    name,
    src: `data:image/svg+xml,${encodeURIComponent(svg)}`,
    bytes: 39_000 + (seed % 12_000)
  };
}
