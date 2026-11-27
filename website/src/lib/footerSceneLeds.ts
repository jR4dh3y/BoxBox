// Typed hardware status LED coordinates and ambient valley dust motes for SiteFooter.
// Each LED coordinate is measured directly against its parent 1600px parallax layer
// so the SVG overlay tracks its hardware bezel with sub-pixel parallax lockstep.

export type LedTone = 'accent' | 'gold';
export type LedPattern = 'steady' | 'io-fast' | 'io-burst' | 'heartbeat';

export interface SceneLed {
  x: number;
  y: number;
  r: number;
  tone: LedTone;
  pattern: LedPattern;
  durationSec: number;
  delaySec: number;
}

export interface AmbientMote {
  leftPct: number;
  bottomPct: number;
  sizePx: number;
  driftX: number;
  risePx: number;
  durationSec: number;
  delaySec: number;
  tone: LedTone;
}

/** Subtle recessed hardware LEDs aligned to `/parallax/scene/racks-far.webp` (1600x549). */
export const farRackLeds: readonly SceneLed[] = [
  // Left tall cabinets
  { x: 142.2, y: 389.8, r: 1.1, tone: 'accent', pattern: 'io-burst', durationSec: 4.9, delaySec: -0.7 },
  { x: 142.1, y: 444.3, r: 1.0, tone: 'gold', pattern: 'io-fast', durationSec: 3.7, delaySec: -1.9 },
  { x: 316.3, y: 100.0, r: 1.1, tone: 'accent', pattern: 'steady', durationSec: 6.4, delaySec: -1.2 },
  { x: 316.7, y: 137.0, r: 1.0, tone: 'accent', pattern: 'io-burst', durationSec: 4.3, delaySec: -0.4 },
  { x: 316.3, y: 173.0, r: 1.0, tone: 'gold', pattern: 'io-fast', durationSec: 3.5, delaySec: -2.1 },

  // Left mid rack vertical blades
  { x: 484.4, y: 281.0, r: 1.0, tone: 'accent', pattern: 'io-fast', durationSec: 3.1, delaySec: -0.3 },
  { x: 484.5, y: 341.0, r: 1.0, tone: 'accent', pattern: 'io-burst', durationSec: 5.1, delaySec: -3.1 },
  { x: 484.4, y: 401.0, r: 1.0, tone: 'gold', pattern: 'heartbeat', durationSec: 5.9, delaySec: -1.5 },

  // Center valley low 1U servers & far NAS bays
  { x: 659.5, y: 384.5, r: 1.05, tone: 'accent', pattern: 'io-burst', durationSec: 4.1, delaySec: -1.1 },
  { x: 675.8, y: 384.8, r: 1.0, tone: 'gold', pattern: 'io-fast', durationSec: 3.2, delaySec: -0.6 },
  { x: 659.5, y: 410.0, r: 1.0, tone: 'accent', pattern: 'steady', durationSec: 6.7, delaySec: -2.2 },
  { x: 962.0, y: 439.5, r: 1.05, tone: 'accent', pattern: 'io-burst', durationSec: 4.5, delaySec: -1.7 },
  { x: 999.5, y: 439.5, r: 1.0, tone: 'gold', pattern: 'io-fast', durationSec: 3.3, delaySec: -0.9 },
  { x: 1048.5, y: 365.5, r: 1.05, tone: 'accent', pattern: 'heartbeat', durationSec: 5.7, delaySec: -0.8 },

  // Right tall cabinets
  { x: 1395.9, y: 100.1, r: 1.1, tone: 'accent', pattern: 'steady', durationSec: 6.9, delaySec: -1.4 },
  { x: 1395.9, y: 137.1, r: 1.0, tone: 'gold', pattern: 'io-fast', durationSec: 3.6, delaySec: -2.6 },
  { x: 1395.9, y: 245.9, r: 1.0, tone: 'accent', pattern: 'io-burst', durationSec: 4.7, delaySec: -1.8 },
  { x: 1569.9, y: 63.9, r: 1.1, tone: 'accent', pattern: 'heartbeat', durationSec: 5.5, delaySec: -2.0 },
  { x: 1570.0, y: 118.3, r: 1.0, tone: 'accent', pattern: 'io-fast', durationSec: 3.4, delaySec: -0.9 },
];

/** Subtle recessed NAS LEDs aligned to `/parallax/scene/midground-bays.webp` (1600x490). */
export const midgroundBayLeds: readonly SceneLed[] = [
  // Left 4-bay NAS drive bay indicators
  { x: 390.6, y: 106.8, r: 1.25, tone: 'accent', pattern: 'io-burst', durationSec: 4.3, delaySec: -0.5 },
  { x: 412.4, y: 106.8, r: 1.25, tone: 'gold', pattern: 'io-fast', durationSec: 3.1, delaySec: -1.3 },
  { x: 434.2, y: 106.8, r: 1.25, tone: 'accent', pattern: 'io-fast', durationSec: 3.7, delaySec: -2.1 },
  { x: 456.0, y: 106.7, r: 1.25, tone: 'accent', pattern: 'io-burst', durationSec: 4.9, delaySec: -0.9 },
  // Left 4-bay NAS side status slits + actual recessed power pinhole
  { x: 495.5, y: 109.5, r: 1.05, tone: 'accent', pattern: 'steady', durationSec: 6.4, delaySec: -1.1 },
  { x: 495.8, y: 117.0, r: 1.05, tone: 'gold', pattern: 'io-fast', durationSec: 2.9, delaySec: -0.4 },
  { x: 495.5, y: 124.5, r: 1.05, tone: 'accent', pattern: 'io-burst', durationSec: 4.1, delaySec: -2.7 },
  { x: 492.6, y: 170.2, r: 1.2, tone: 'accent', pattern: 'steady', durationSec: 7.2, delaySec: -2.0 },

  // Right 4-bay NAS drive bay indicators
  { x: 1140.2, y: 106.8, r: 1.25, tone: 'accent', pattern: 'io-fast', durationSec: 3.4, delaySec: -1.6 },
  { x: 1161.7, y: 107.0, r: 1.25, tone: 'accent', pattern: 'io-burst', durationSec: 4.5, delaySec: -0.3 },
  { x: 1183.4, y: 106.9, r: 1.25, tone: 'gold', pattern: 'io-fast', durationSec: 3.0, delaySec: -2.2 },
  { x: 1205.5, y: 107.5, r: 1.25, tone: 'accent', pattern: 'io-burst', durationSec: 3.9, delaySec: -1.1 },
  // Right 4-bay NAS side status slits + actual recessed power pinhole
  { x: 1245.0, y: 109.8, r: 1.05, tone: 'accent', pattern: 'steady', durationSec: 6.7, delaySec: -3.2 },
  { x: 1245.0, y: 117.0, r: 1.05, tone: 'accent', pattern: 'io-fast', durationSec: 3.2, delaySec: -0.8 },
  { x: 1245.2, y: 124.6, r: 1.05, tone: 'gold', pattern: 'io-burst', durationSec: 4.3, delaySec: -1.9 },
  { x: 1243.2, y: 170.2, r: 1.2, tone: 'accent', pattern: 'steady', durationSec: 7.4, delaySec: -0.7 },
];

/** Subtle recessed rack LEDs aligned to `/parallax/scene/foreground-frame.webp` (1600x215). */
export const foregroundFrameLeds: readonly SceneLed[] = [
  // Left foreground rack stack
  { x: 302.5, y: 51.0, r: 1.2, tone: 'accent', pattern: 'io-burst', durationSec: 4.3, delaySec: -0.6 },
  { x: 318.0, y: 51.5, r: 1.15, tone: 'gold', pattern: 'io-fast', durationSec: 3.1, delaySec: -1.4 },
  { x: 302.5, y: 76.5, r: 1.15, tone: 'accent', pattern: 'steady', durationSec: 6.5, delaySec: -2.8 },
  { x: 302.5, y: 101.5, r: 1.15, tone: 'gold', pattern: 'io-burst', durationSec: 4.7, delaySec: -2.1 },
  { x: 319.5, y: 125.5, r: 1.15, tone: 'accent', pattern: 'heartbeat', durationSec: 5.7, delaySec: -1.2 },

  // Right foreground 6-bay NAS #1
  { x: 1289.5, y: 105.5, r: 1.2, tone: 'accent', pattern: 'io-fast', durationSec: 3.2, delaySec: -0.4 },
  { x: 1327.5, y: 105.5, r: 1.2, tone: 'gold', pattern: 'io-fast', durationSec: 2.9, delaySec: -0.9 },
  { x: 1345.5, y: 105.5, r: 1.2, tone: 'accent', pattern: 'io-burst', durationSec: 4.1, delaySec: -2.6 },
  { x: 1383.5, y: 105.5, r: 1.2, tone: 'gold', pattern: 'io-burst', durationSec: 4.9, delaySec: -3.1 },
  { x: 1418.5, y: 157.5, r: 1.15, tone: 'accent', pattern: 'steady', durationSec: 6.9, delaySec: -2.3 },

  // Right foreground 6-bay NAS #2 + top blade
  { x: 1464.0, y: 105.5, r: 1.2, tone: 'accent', pattern: 'io-burst', durationSec: 4.2, delaySec: -1.1 },
  { x: 1502.0, y: 105.5, r: 1.2, tone: 'accent', pattern: 'io-fast', durationSec: 3.5, delaySec: -0.5 },
  { x: 1540.0, y: 105.5, r: 1.2, tone: 'gold', pattern: 'io-fast', durationSec: 3.0, delaySec: -1.3 },
  { x: 1541.7, y: 31.3, r: 1.15, tone: 'accent', pattern: 'heartbeat', durationSec: 5.4, delaySec: -0.7 },
  { x: 1592.6, y: 157.2, r: 1.15, tone: 'accent', pattern: 'steady', durationSec: 7.3, delaySec: -3.5 },
];

/** Subtle rising ambient motes drifting lazily through the central valley horizon. */
export const ambientValleyMotes: readonly AmbientMote[] = [
  { leftPct: 26, bottomPct: 18, sizePx: 2.0, driftX: 14, risePx: 115, durationSec: 11.5, delaySec: -2.1, tone: 'accent' },
  { leftPct: 31, bottomPct: 12, sizePx: 1.5, driftX: -12, risePx: 130, durationSec: 13.8, delaySec: -7.4, tone: 'gold' },
  { leftPct: 36, bottomPct: 22, sizePx: 2.2, driftX: 18, risePx: 120, durationSec: 10.6, delaySec: -4.2, tone: 'accent' },
  { leftPct: 40, bottomPct: 14, sizePx: 1.6, driftX: -10, risePx: 145, durationSec: 14.4, delaySec: -9.5, tone: 'accent' },
  { leftPct: 44, bottomPct: 26, sizePx: 2.0, driftX: 12, risePx: 108, durationSec: 12.2, delaySec: -1.8, tone: 'gold' },
  { leftPct: 48, bottomPct: 16, sizePx: 2.4, driftX: -16, risePx: 138, durationSec: 11.1, delaySec: -6.3, tone: 'accent' },
  { leftPct: 51, bottomPct: 20, sizePx: 1.8, driftX: 15, risePx: 126, durationSec: 13.2, delaySec: -3.7, tone: 'accent' },
  { leftPct: 54, bottomPct: 13, sizePx: 2.0, driftX: -14, risePx: 142, durationSec: 12.7, delaySec: -10.1, tone: 'gold' },
  { leftPct: 58, bottomPct: 24, sizePx: 1.6, driftX: 11, risePx: 112, durationSec: 10.9, delaySec: -5.4, tone: 'accent' },
  { leftPct: 62, bottomPct: 15, sizePx: 2.2, driftX: -18, risePx: 134, durationSec: 14.1, delaySec: -8.2, tone: 'accent' },
  { leftPct: 66, bottomPct: 21, sizePx: 1.5, driftX: 13, risePx: 118, durationSec: 11.8, delaySec: -2.9, tone: 'gold' },
  { leftPct: 71, bottomPct: 14, sizePx: 2.0, driftX: -11, risePx: 128, durationSec: 13.5, delaySec: -6.8, tone: 'accent' },
  { leftPct: 75, bottomPct: 19, sizePx: 1.8, driftX: 16, risePx: 105, durationSec: 12.4, delaySec: -4.9, tone: 'accent' },
];
