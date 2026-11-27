// Interactive ordered-Bayer dot-grid backdrop for the top hero zone of every page.
// Keeps every dot locked to the exact 22px architectural grid (zero spotlight blobs,
// zero dot displacement) while lighting crisp Bayer-dithered trails and click ripples
// in the active --color-accent theme. Sleeps automatically at 0% CPU when settled.

import { clamp } from './footerParallax';
import {
  bayerThreshold,
  dotProximity,
  mixRgb,
  parseCssColor,
  type RgbColor,
} from './footerHalftone';

export type GridDotMode = 'rest' | 'whisper' | 'lit';

interface GridRipple {
  x: number;
  y: number;
  radius: number;
  strength: number;
}

const GRID_STEP = 22;
const HOVER_RADIUS = 176;
const INTRO_DURATION_SEC = 1.65;
const WHITE: RgbColor = { r: 245, g: 249, b: 255 };

/** Vertical edge fade matching the original CSS dot-grid linear mask (1 at top -> 0 at bottom). */
export function verticalGridFade(y: number, height: number): number {
  if (height <= 0) return 0;
  return clamp(1 - y / height, 0, 1);
}

/**
 * Classifies a dot's discrete dither illumination mode from its decaying trail
 * `energy` in [0, 1] against its 4x4 Bayer `threshold`: dots snap cleanly ON in
 * ordered checkerboard steps instead of rendering a blurry circular spotlight.
 */
export function gridDotMode(energy: number, threshold: number): GridDotMode {
  if (energy <= 0.04) return 'rest';
  return energy >= threshold * 0.72 ? 'lit' : 'whisper';
}

/** Gentle welcoming diagonal sweep in [0, 1] on first load, returning 0 once finished. */
export function introSweepEnergy(nx: number, ny: number, elapsedSec: number): number {
  if (elapsedSec < 0 || elapsedSec > INTRO_DURATION_SEC) return 0;
  const progress = elapsedSec / INTRO_DURATION_SEC;
  const sweepPos = -0.2 + progress * 1.5;
  const diag = nx * 0.85 + ny * 0.25;
  const dist = Math.abs(diag - sweepPos);
  if (dist >= 0.16) return 0;
  const band = 1 - dist / 0.16;
  return band * band * 0.72 * (1 - progress * 0.35);
}

export function mountInteractiveDotGrid(container: HTMLElement): void {
  const motion = window.matchMedia('(prefers-reduced-motion: reduce)');
  if (motion.matches) return;

  const canvas = container.querySelector<HTMLCanvasElement>('[data-dot-grid]');
  if (!canvas) return;
  const canvasEl = canvas;
  const ctx = canvasEl.getContext('2d');
  if (!ctx) return;
  const context = ctx;

  let width = 0;
  let height = 0;
  let dpr = 1;
  let cols = 0;
  let rows = 0;
  let energies = new Float32Array(0);
  let thresholds = new Float32Array(0);

  let targetAccent = parseCssColor(
    getComputedStyle(document.documentElement).getPropertyValue('--color-accent')
  );
  let currentAccent: RgbColor = { ...targetAccent };

  let visible = true;
  let frame = 0;
  let lastTime = 0;
  let introElapsed = 0;
  let introDone = false;

  let pointerActive = false;
  let lastPointerX = 0;
  let lastPointerY = 0;
  const ripples: GridRipple[] = [];

  function readAccent(): void {
    targetAccent = parseCssColor(
      getComputedStyle(document.documentElement).getPropertyValue('--color-accent')
    );
  }

  function buildGrid(): void {
    const rect = container.getBoundingClientRect();
    const nextW = Math.round(rect.width);
    const nextH = Math.round(rect.height);
    if (nextW < 20 || nextH < 20) return;

    dpr = Math.min(window.devicePixelRatio || 1, 2);
    width = nextW;
    height = nextH;
    canvasEl.width = Math.round(width * dpr);
    canvasEl.height = Math.round(height * dpr);

    cols = Math.ceil(width / GRID_STEP);
    rows = Math.ceil(height / GRID_STEP);
    const total = cols * rows;
    energies = new Float32Array(total);
    thresholds = new Float32Array(total);

    for (let r = 0; r < rows; r += 1) {
      for (let c = 0; c < cols; c += 1) {
        thresholds[r * cols + c] = bayerThreshold(c, r);
      }
    }

    container.dataset.dotGridReady = 'true';
    draw(1 / 60, true);
    if (!introDone && visible) wake();
  }

  function chargeSegment(x0: number, y0: number, x1: number, y1: number): void {
    if (cols <= 0 || rows <= 0) return;
    const minC = Math.max(0, Math.floor((Math.min(x0, x1) - HOVER_RADIUS) / GRID_STEP));
    const maxC = Math.min(cols - 1, Math.ceil((Math.max(x0, x1) + HOVER_RADIUS) / GRID_STEP));
    const minR = Math.max(0, Math.floor((Math.min(y0, y1) - HOVER_RADIUS) / GRID_STEP));
    const maxR = Math.min(rows - 1, Math.ceil((Math.max(y0, y1) + HOVER_RADIUS) / GRID_STEP));

    const sx = x1 - x0;
    const sy = y1 - y0;
    const segLenSq = sx * sx + sy * sy;

    for (let r = minR; r <= maxR; r += 1) {
      const py = r * GRID_STEP + GRID_STEP * 0.5;
      const rowOffset = r * cols;
      for (let c = minC; c <= maxC; c += 1) {
        const px = c * GRID_STEP + GRID_STEP * 0.5;
        let t = 0;
        if (segLenSq > 0.001) {
          t = clamp(((px - x0) * sx + (py - y0) * sy) / segLenSq, 0, 1);
        }
        const cx = x0 + sx * t;
        const cy = y0 + sy * t;
        const dist = Math.hypot(px - cx, py - cy);
        const prox = dotProximity(dist, HOVER_RADIUS);
        if (prox > 0.01) {
          const idx = rowOffset + c;
          const next = Math.min(1, prox * 1.05);
          if (next > (energies[idx] ?? 0)) energies[idx] = next;
        }
      }
    }
  }

  function draw(dt: number, staticPass = false): void {
    context.setTransform(dpr, 0, 0, dpr, 0, 0);
    context.clearRect(0, 0, width, height);

    const stepDt = clamp(dt, 1 / 240, 0.05);
    let stillActive = false;

    if (staticPass) {
      currentAccent = { ...targetAccent };
    } else {
      const colorLerp = 1 - Math.exp(-stepDt * 5.2);
      const dr = targetAccent.r - currentAccent.r;
      const dg = targetAccent.g - currentAccent.g;
      const db = targetAccent.b - currentAccent.b;
      if (Math.abs(dr) + Math.abs(dg) + Math.abs(db) > 1.5) {
        currentAccent = {
          r: currentAccent.r + dr * colorLerp,
          g: currentAccent.g + dg * colorLerp,
          b: currentAccent.b + db * colorLerp,
        };
        stillActive = true;
      } else {
        currentAccent = { ...targetAccent };
      }

      if (!introDone) {
        introElapsed += stepDt;
        if (introElapsed > INTRO_DURATION_SEC) {
          introDone = true;
        } else {
          stillActive = true;
        }
      }

      const rippleDecay = Math.exp(-stepDt * 2.6);
      for (let i = ripples.length - 1; i >= 0; i -= 1) {
        const ripple = ripples[i];
        if (!ripple) continue;
        ripple.radius += 540 * stepDt;
        ripple.strength *= rippleDecay;
        if (ripple.strength < 0.035 || ripple.radius > Math.max(width, height) * 0.9) {
          ripples.splice(i, 1);
        } else {
          stillActive = true;
        }
      }
    }

    const decay = Math.exp(-stepDt * 3.4);
    const accentCore = mixRgb(currentAccent, WHITE, 0.28);
    const accentPeak = mixRgb(currentAccent, WHITE, 0.65);

    for (let r = 0; r < rows; r += 1) {
      const y = r * GRID_STEP + GRID_STEP * 0.5;
      const vFade = verticalGridFade(y, height);
      if (vFade <= 0.008) continue;

      const ny = y / height;
      const rowOffset = r * cols;

      for (let c = 0; c < cols; c += 1) {
        const x = c * GRID_STEP + GRID_STEP * 0.5;
        const idx = rowOffset + c;
        let energy = energies[idx] ?? 0;

        if (!staticPass) {
          if (energy > 0) {
            energy *= decay;
            if (energy < 0.008) {
              energy = 0;
            } else {
              stillActive = true;
            }
            energies[idx] = energy;
          }

          if (!introDone) {
            const sweep = introSweepEnergy(x / width, ny, introElapsed);
            if (sweep > energy) energy = sweep;
          }

          if (ripples.length > 0) {
            for (let ri = 0; ri < ripples.length; ri += 1) {
              const ripple = ripples[ri];
              if (!ripple) continue;
              const dist = Math.hypot(x - ripple.x, y - ripple.y);
              const band = 1 - Math.abs(dist - ripple.radius) / 72;
              if (band > 0) {
                const wave = band * band * ripple.strength;
                if (wave > energy) energy = wave;
              }
            }
          }
        }

        const mode = gridDotMode(energy, thresholds[idx] ?? 0.5);

        if (mode === 'rest') {
          const alpha = 0.095 * vFade;
          context.fillStyle = `rgba(255, 255, 255, ${alpha.toFixed(3)})`;
          context.fillRect(x - 0.5, y - 0.5, 1.15, 1.15);
          continue;
        }

        if (mode === 'whisper') {
          const alpha = (0.11 + energy * 0.14) * vFade;
          context.fillStyle = `rgba(${currentAccent.r}, ${currentAccent.g}, ${currentAccent.b}, ${alpha.toFixed(3)})`;
          context.fillRect(x - 0.6, y - 0.6, 1.3, 1.3);
          continue;
        }

        // Crisp ordered-Bayer ON dot in the active theme accent
        const boost = clamp(energy, 0, 1);
        const color = boost > 0.65 ? accentPeak : accentCore;
        const alpha = clamp((0.28 + boost * 0.56) * vFade, 0.05, 0.86);
        const radius = 1.05 + boost * 0.72;

        context.fillStyle = `rgba(${color.r}, ${color.g}, ${color.b}, ${alpha.toFixed(3)})`;
        context.beginPath();
        context.arc(x, y, radius, 0, Math.PI * 2);
        context.fill();
      }
    }

    if (!staticPass && visible && stillActive) {
      frame = window.requestAnimationFrame(stepLoop);
    } else {
      frame = 0;
      lastTime = 0;
    }
  }

  function stepLoop(now: number): void {
    frame = 0;
    if (motion.matches || !visible) {
      lastTime = 0;
      return;
    }
    const dt = lastTime > 0 ? (now - lastTime) / 1000 : 1 / 60;
    lastTime = now;
    draw(dt, false);
  }

  function wake(): void {
    if (!motion.matches && visible && frame === 0) {
      frame = window.requestAnimationFrame(stepLoop);
    }
  }

  function toLocalCoords(event: PointerEvent): { x: number; y: number } {
    const rect = container.getBoundingClientRect();
    return {
      x: event.clientX - rect.left,
      y: event.clientY - rect.top,
    };
  }

  window.addEventListener(
    'pointermove',
    (event) => {
      if (event.pointerType !== 'mouse' || motion.matches || !visible || width <= 0) return;
      const local = toLocalCoords(event);
      const inside =
        local.x >= -HOVER_RADIUS &&
        local.x <= width + HOVER_RADIUS &&
        local.y >= -HOVER_RADIUS &&
        local.y <= height + HOVER_RADIUS;

      if (!inside) {
        pointerActive = false;
        return;
      }

      if (!pointerActive) {
        lastPointerX = local.x;
        lastPointerY = local.y;
      }
      pointerActive = true;
      chargeSegment(lastPointerX, lastPointerY, local.x, local.y);
      lastPointerX = local.x;
      lastPointerY = local.y;
      wake();
    },
    { passive: true }
  );

  window.addEventListener('pointerleave', () => {
    pointerActive = false;
  });

  window.addEventListener(
    'pointerdown',
    (event) => {
      if (motion.matches || !visible || width <= 0) return;
      const local = toLocalCoords(event);
      if (local.y < 0 || local.y > height) return;
      ripples.push({ x: local.x, y: local.y, radius: 14, strength: 0.92 });
      wake();
    },
    { passive: true }
  );

  const visibilityObserver = new IntersectionObserver(
    ([entry]) => {
      visible = entry?.isIntersecting ?? false;
      if (!visible && frame !== 0) {
        window.cancelAnimationFrame(frame);
        frame = 0;
        lastTime = 0;
      } else if (visible && !introDone) {
        wake();
      }
    },
    { rootMargin: '120px 0px' }
  );
  visibilityObserver.observe(container);

  const resizeObserver = new ResizeObserver(buildGrid);
  resizeObserver.observe(container);

  const themeObserver = new MutationObserver(() => {
    readAccent();
    wake();
  });
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['style'] });

  motion.addEventListener('change', () => {
    if (motion.matches) {
      if (frame !== 0) window.cancelAnimationFrame(frame);
      frame = 0;
      lastTime = 0;
      energies.fill(0);
      ripples.length = 0;
      delete container.dataset.dotGridReady;
      context.clearRect(0, 0, width, height);
    } else {
      buildGrid();
    }
  });

  buildGrid();
}
