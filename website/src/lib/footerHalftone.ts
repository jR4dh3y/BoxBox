// Interactive ordered-dither halftone renderer for the monumental BoxBox wordmark.
// Draws the exact BoxBox glyph geometry offscreen to sample a fixed dot matrix,
// toggles dots cleanly ON and OFF through a 4x4 Bayer dither threshold on hover/click,
// and runs a quiet periodic ambient dither sweep while visible in the viewport.

import { clamp } from './footerParallax';

export interface RgbColor {
  r: number;
  g: number;
  b: number;
}

export type DitherState = 'rest' | 'lit' | 'off';

interface HalftoneDot {
  bx: number;
  by: number;
  nx: number;
  ny: number;
  baseRadius: number;
  threshold: number;
  level: number;
  fade: number;
}

interface Ripple {
  x: number;
  y: number;
  radius: number;
  strength: number;
}

const WORDMARK_W = 930;
const WORDMARK_H = 256;
const SECOND_BOX_X = 448;
const DEFAULT_ACCENT: RgbColor = { r: 74, g: 158, b: 255 };
const AMBIENT_CYCLE_SEC = 7.4;

const BAYER_4X4: readonly (readonly number[])[] = [
  [0, 8, 2, 10],
  [12, 4, 14, 6],
  [3, 11, 1, 9],
  [15, 7, 13, 5],
];

export function parseCssColor(value: string): RgbColor {
  const trimmed = value.trim();
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(trimmed);
  if (hex) {
    const raw = hex[1] ?? '';
    const full = raw.length === 3 ? raw.split('').map((c) => c + c).join('') : raw;
    const n = Number.parseInt(full, 16);
    return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
  }

  const rgb = /^rgba?\(\s*(\d+)\s*[,\s]\s*(\d+)\s*[,\s]\s*(\d+)/i.exec(trimmed);
  if (rgb) {
    return {
      r: clamp(Number(rgb[1]), 0, 255),
      g: clamp(Number(rgb[2]), 0, 255),
      b: clamp(Number(rgb[3]), 0, 255),
    };
  }

  return DEFAULT_ACCENT;
}

export function mixRgb(a: RgbColor, b: RgbColor, t: number): RgbColor {
  const k = clamp(t, 0, 1);
  return {
    r: Math.round(a.r + (b.r - a.r) * k),
    g: Math.round(a.g + (b.g - a.g) * k),
    b: Math.round(a.b + (b.b - a.b) * k),
  };
}

/** Smooth quadratic falloff in [0, 1] for a dot at `distance` inside `radius`. */
export function dotProximity(distance: number, radius: number): number {
  if (radius <= 0 || distance >= radius) return 0;
  const linear = 1 - Math.max(0, distance) / radius;
  return linear * linear;
}

/** Canonical normalized 4x4 ordered Bayer threshold in (0, 1) for cell `(col, row)`. */
export function bayerThreshold(col: number, row: number): number {
  const r = ((row % 4) + 4) % 4;
  const c = ((col % 4) + 4) % 4;
  return ((BAYER_4X4[r]?.[c] ?? 0) + 0.5) / 16;
}

/**
 * Resolves a dot's discrete dither state from its active hover/wave `level`
 * against its Bayer `threshold`: dots stay calm at rest, brighten ON along the
 * dither frontier, and snap cleanly OFF once `level` crosses `threshold`.
 */
export function ditherDotState(level: number, threshold: number): DitherState {
  if (level <= 0.04) return 'rest';
  return level >= threshold ? 'off' : 'lit';
}

/**
 * Computes a gentle diagonal ambient telemetry sheen in [0, 1] for normalized
 * position `(nx, ny)` at elapsed `timeSec`. Spends most of each cycle quiet so
 * the wordmark stays calm and readable while feeling subtly alive.
 */
export function ambientSweepStrength(nx: number, ny: number, timeSec: number): number {
  const phase = ((timeSec % AMBIENT_CYCLE_SEC) + AMBIENT_CYCLE_SEC) % AMBIENT_CYCLE_SEC;
  // Active sweep takes 2.2s, followed by 5.2s of calm rest
  if (phase > 2.2) return 0;
  const sweepPos = -0.22 + (phase / 2.2) * 1.54;
  const diag = nx + ny * 0.22;
  const dist = Math.abs(diag - sweepPos);
  if (dist >= 0.14) return 0;
  const band = 1 - dist / 0.14;
  return band * band;
}

function drawHalfBox(ctx: CanvasRenderingContext2D, offsetX: number): void {
  ctx.save();
  ctx.translate(offsetX, 0);

  // "B" upper and lower capsules (flat left, 20px right corners)
  ctx.fillStyle = '#fff';
  ctx.beginPath();
  ctx.roundRect(40, 40, 104, 84, [0, 20, 20, 0]);
  ctx.roundRect(40, 136, 128, 84, [0, 20, 20, 0]);
  ctx.fill();

  // Server bay cutouts inside "B"
  ctx.globalCompositeOperation = 'destination-out';
  ctx.beginPath();
  ctx.roundRect(80, 68, 40, 28, 8);
  ctx.roundRect(80, 160, 64, 28, 8);
  ctx.arc(58, 82, 6, 0, Math.PI * 2);
  ctx.moveTo(64, 174);
  ctx.arc(58, 174, 6, 0, Math.PI * 2);
  ctx.fill();

  // "o" rounded square and center pin
  ctx.globalCompositeOperation = 'source-over';
  ctx.strokeStyle = '#fff';
  ctx.lineWidth = 28;
  ctx.beginPath();
  ctx.roundRect(206, 110, 92, 92, 26);
  ctx.stroke();

  ctx.beginPath();
  ctx.arc(252, 156, 9, 0, Math.PI * 2);
  ctx.fill();

  // "x" crossed strokes
  ctx.lineCap = 'round';
  ctx.beginPath();
  ctx.moveTo(348, 110);
  ctx.lineTo(428, 202);
  ctx.moveTo(428, 110);
  ctx.lineTo(348, 202);
  ctx.stroke();

  ctx.restore();
}

function buildWordmarkDots(width: number, height: number): { dots: HalftoneDot[]; step: number } {
  const offscreen = document.createElement('canvas');
  offscreen.width = Math.max(1, Math.round(width));
  offscreen.height = Math.max(1, Math.round(height));

  const ctx = offscreen.getContext('2d', { willReadFrequently: true });
  if (!ctx) return { dots: [], step: 7 };

  const scale = Math.min(offscreen.width / WORDMARK_W, offscreen.height / WORDMARK_H);
  const offsetX = (offscreen.width - WORDMARK_W * scale) * 0.5;
  const offsetY = (offscreen.height - WORDMARK_H * scale) * 0.5;

  ctx.save();
  ctx.translate(offsetX, offsetY);
  ctx.scale(scale, scale);
  drawHalfBox(ctx, 0);
  drawHalfBox(ctx, SECOND_BOX_X);
  ctx.restore();

  const pixels = ctx.getImageData(0, 0, offscreen.width, offscreen.height).data;
  const step = clamp(width / 80, 5.2, 8.2);
  const dots: HalftoneDot[] = [];

  for (let row = 0, y = step * 0.5; y < offscreen.height; row += 1, y += step) {
    const ny = y / offscreen.height;
    // Gentle halftone taper toward the bottom edge so it seats cleanly into the valley
    const fade = ny < 0.72 ? 1 : clamp(1 - (ny - 0.72) / 0.34, 0.28, 1);

    for (let col = 0, x = step * 0.5; x < offscreen.width; col += 1, x += step) {
      const idx = (Math.floor(y) * offscreen.width + Math.floor(x)) * 4 + 3;
      const coverage = (pixels[idx] ?? 0) / 255;
      if (coverage < 0.42) continue;

      const baseRadius = step * (0.24 + 0.08 * coverage) * (0.72 + 0.28 * fade);
      dots.push({
        bx: x,
        by: y,
        nx: x / offscreen.width,
        ny,
        baseRadius,
        threshold: bayerThreshold(col, row),
        level: 0,
        fade,
      });
    }
  }

  return { dots, step };
}

export function mountFooterHalftone(canvas: HTMLCanvasElement, interactionRoot: HTMLElement): void {
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  const context = ctx;

  const motion = window.matchMedia('(prefers-reduced-motion: reduce)');
  let dots: HalftoneDot[] = [];
  let width = 0;
  let height = 0;
  let dpr = 1;

  let targetAccent = parseCssColor(
    getComputedStyle(interactionRoot).getPropertyValue('--color-accent')
  );
  let currentAccent: RgbColor = { ...targetAccent };
  let frame = 0;
  let lastTime = 0;
  let elapsedSec = 0;
  let visible = false;

  let pointerActive = false;
  let pointerX = 0;
  let pointerY = 0;
  let smoothPointerX = 0;
  let smoothPointerY = 0;
  const ripples: Ripple[] = [];

  function readAccent(): void {
    targetAccent = parseCssColor(
      getComputedStyle(interactionRoot).getPropertyValue('--color-accent')
    );
  }

  function resize(): void {
    const nextW = Math.round(canvas.offsetWidth);
    const nextH = Math.round(canvas.offsetHeight);
    if (nextW < 40 || nextH < 20) return;

    dpr = Math.min(window.devicePixelRatio || 1, 2);
    width = nextW;
    height = nextH;
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);

    const built = buildWordmarkDots(width, height);
    dots = built.dots;
    draw(true);
    if (visible && !motion.matches) wake();
  }

  function draw(staticPass = false, dt = 1 / 60): void {
    context.setTransform(dpr, 0, 0, dpr, 0, 0);
    context.clearRect(0, 0, width, height);

    let stillMoving = false;
    const stepDt = clamp(dt, 1 / 240, 0.05);
    const colorLerp = 1 - Math.exp(-stepDt * 5.2);
    const pointerLerp = 1 - Math.exp(-stepDt * 22);
    const attackRate = 1 - Math.exp(-stepDt * 24);
    const decayRate = 1 - Math.exp(-stepDt * 5.5);
    const rippleDecay = Math.exp(-stepDt * 3.8);

    if (staticPass) {
      currentAccent = { ...targetAccent };
    } else {
      elapsedSec += stepDt;

      const dr = targetAccent.r - currentAccent.r;
      const dg = targetAccent.g - currentAccent.g;
      const db = targetAccent.b - currentAccent.b;
      if (Math.abs(dr) + Math.abs(dg) + Math.abs(db) > 1.5) {
        currentAccent = {
          r: currentAccent.r + dr * colorLerp,
          g: currentAccent.g + dg * colorLerp,
          b: currentAccent.b + db * colorLerp,
        };
        stillMoving = true;
      } else {
        currentAccent = { ...targetAccent };
      }

      smoothPointerX += (pointerX - smoothPointerX) * pointerLerp;
      smoothPointerY += (pointerY - smoothPointerY) * pointerLerp;
      if (
        pointerActive &&
        Math.abs(pointerX - smoothPointerX) + Math.abs(pointerY - smoothPointerY) > 0.25
      ) {
        stillMoving = true;
      }

      for (let i = ripples.length - 1; i >= 0; i -= 1) {
        const ripple = ripples[i];
        if (!ripple) continue;
        ripple.radius += 660 * stepDt;
        ripple.strength *= rippleDecay;
        if (ripple.strength < 0.03 || ripple.radius > width * 1.2) {
          ripples.splice(i, 1);
        } else {
          stillMoving = true;
        }
      }
    }

    const influence = Math.max(105, width * 0.22);
    const white: RgbColor = { r: 245, g: 248, b: 255 };
    const restTint = mixRgb(currentAccent, white, 0.22);
    const litTint = mixRgb(currentAccent, white, 0.72);
    const sweepActive = !staticPass && visible;

    for (let i = 0; i < dots.length; i += 1) {
      const dot = dots[i];
      if (!dot) continue;

      let targetLevel = 0;

      if (!staticPass && pointerActive) {
        const dist = Math.hypot(dot.bx - smoothPointerX, dot.by - smoothPointerY);
        const prox = dotProximity(dist, influence);
        if (prox > 0.001) {
          targetLevel = Math.max(targetLevel, Math.min(1, prox * 1.15));
        }
      }

      if (!staticPass && ripples.length > 0) {
        for (let rIdx = 0; rIdx < ripples.length; rIdx += 1) {
          const ripple = ripples[rIdx];
          if (!ripple) continue;
          const dist = Math.hypot(dot.bx - ripple.x, dot.by - ripple.y);
          const band = 1 - Math.abs(dist - ripple.radius) / 54;
          if (band > 0) {
            targetLevel = Math.max(targetLevel, Math.min(1, band * band * ripple.strength * 1.2));
          }
        }
      }

      if (staticPass) {
        dot.level = 0;
      } else {
        // Fast attack so pixels turn OFF crisply under the cursor, slower decay so
        // they step back ON in visible 4x4 Bayer dither stages as the cursor leaves.
        const rate = targetLevel > dot.level ? attackRate : decayRate;
        dot.level += (targetLevel - dot.level) * rate;
        if (Math.abs(targetLevel - dot.level) > 0.004) {
          stillMoving = true;
        } else {
          dot.level = targetLevel;
        }
      }

      const state = ditherDotState(dot.level, dot.threshold);
      if (state === 'off') continue;

      // Subtle periodic ordered-dither telemetry sweep when idle and visible
      let shimmer = 0;
      if (sweepActive && state === 'rest') {
        const sweep = ambientSweepStrength(dot.nx, dot.ny, elapsedSec);
        if (sweep > 0.05) {
          // Only light dots whose Bayer threshold falls inside the sweep band
          if (sweep * 0.65 > dot.threshold * 0.55) {
            shimmer = sweep * 0.65;
          }
        }
      }

      const color =
        state === 'lit'
          ? litTint
          : shimmer > 0
            ? mixRgb(restTint, litTint, shimmer)
            : restTint;

      const alpha =
        state === 'lit'
          ? clamp((0.68 + dot.level * 0.28) * dot.fade, 0.24, 0.96)
          : clamp((0.42 + shimmer * 0.34) * dot.fade, 0.14, 0.82);

      context.fillStyle = `rgba(${color.r}, ${color.g}, ${color.b}, ${alpha.toFixed(3)})`;
      context.beginPath();
      context.arc(dot.bx, dot.by, dot.baseRadius, 0, Math.PI * 2);
      context.fill();
    }

    if (staticPass) return;

    if (visible && (stillMoving || !motion.matches)) {
      if (frame === 0) frame = window.requestAnimationFrame(stepLoop);
    } else {
      frame = 0;
      lastTime = 0;
    }
  }

  function stepLoop(now: number): void {
    frame = 0;
    if (motion.matches || !visible) {
      lastTime = 0;
      if (motion.matches) draw(true);
      return;
    }
    const dt = lastTime > 0 ? (now - lastTime) / 1000 : 1 / 60;
    lastTime = now;
    draw(false, dt);
  }

  function wake(): void {
    if (!motion.matches && visible && frame === 0) {
      frame = window.requestAnimationFrame(stepLoop);
    }
  }

  function toLocalCoords(event: PointerEvent): { x: number; y: number } {
    const rect = canvas.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0 || width <= 0 || height <= 0) {
      return { x: -1e6, y: -1e6 };
    }
    return {
      x: (event.clientX - rect.left) * (width / rect.width),
      y: (event.clientY - rect.top) * (height / rect.height),
    };
  }

  interactionRoot.addEventListener('pointermove', (event) => {
    if (event.pointerType !== 'mouse' || motion.matches || width <= 0) return;
    const local = toLocalCoords(event);
    const pad = Math.max(110, width * 0.24);
    const insideReach =
      local.x >= -pad && local.x <= width + pad && local.y >= -pad && local.y <= height + pad;

    if (!pointerActive && insideReach) {
      smoothPointerX = local.x;
      smoothPointerY = local.y;
    }

    pointerActive = insideReach;
    pointerX = local.x;
    pointerY = local.y;
    wake();
  });

  interactionRoot.addEventListener('pointerleave', () => {
    pointerActive = false;
    wake();
  });

  interactionRoot.addEventListener('pointerdown', (event) => {
    if (motion.matches || width <= 0) return;
    if (event.target instanceof Element && event.target.closest('a, button')) return;
    const local = toLocalCoords(event);
    if (local.x < -80 || local.x > width + 80 || local.y < -80 || local.y > height + 80) return;

    ripples.push({ x: local.x, y: local.y, radius: 10, strength: 1 });
    wake();
  });

  // Pause the idle ambient shimmer whenever the footer is offscreen or covered by the page curtain
  function syncVisibility(): void {
    const nextVisible = interactionRoot.dataset.sceneVisible !== 'false';
    visible = nextVisible;
    if (visible && !motion.matches) {
      wake();
    } else if (!visible && frame !== 0) {
      window.cancelAnimationFrame(frame);
      frame = 0;
      lastTime = 0;
    }
  }

  const visibilityObserver = new MutationObserver(syncVisibility);
  visibilityObserver.observe(interactionRoot, {
    attributes: true,
    attributeFilter: ['data-scene-visible'],
  });
  syncVisibility();

  const resizeObserver = new ResizeObserver(resize);
  resizeObserver.observe(canvas);

  // Smoothly cross-fade dot tint whenever BoxBox's homepage theme cycle updates --color-accent
  const themeObserver = new MutationObserver(() => {
    readAccent();
    if (motion.matches) {
      draw(true);
    } else {
      wake();
    }
  });
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['style'] });

  motion.addEventListener('change', () => {
    if (motion.matches) {
      if (frame !== 0) window.cancelAnimationFrame(frame);
      frame = 0;
      lastTime = 0;
      pointerActive = false;
      ripples.length = 0;
      draw(true);
    } else if (visible) {
      wake();
    }
  });

  resize();
}
