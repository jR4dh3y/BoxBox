// Frame-rate-independent paper physics for the taped sticky note in Closing.astro.
// Hovering peels the loose bottom corners toward the cursor; picking it up tracks
// the pointer tightly with real pendulum sway from velocity & acceleration; releasing
// presses it cleanly back onto the page where it lands.

export const DEFAULT_REST_ANGLE = 2.5;
const MAX_SWING_DEG = 22;
const BOTTOM_INSET_PX = 16;

export interface StickyPhysicsState {
  x: number;
  y: number;
  vx: number;
  vy: number;
  angle: number;
  angleVel: number;
  restAngle: number;
  tiltX: number;
  tiltY: number;
  lift: number;
}

export interface StickyPhysicsTargets {
  x: number;
  y: number;
  tiltX: number;
  tiltY: number;
  lift: number;
  held: boolean;
  reduceMotion: boolean;
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

/**
 * Computes natural corner-peel tilt angles for a note taped along its top edge.
 * Hovering near the top tape barely moves the sheet; hovering near the loose bottom
 * corners lifts that corner toward the viewer (CSS: +rotateX lifts bottom, -rotateY lifts right).
 */
export function computeHoverPeel(
  across: number,
  down: number
): { tiltX: number; tiltY: number; lift: number } {
  const nx = clamp(across, 0, 1);
  const ny = clamp(down, 0, 1);
  const peel = ny * ny;
  return {
    tiltX: 1.5 + peel * 9.5,
    tiltY: (0.5 - nx) * (4 + peel * 12),
    lift: 0.1 + peel * 0.28,
  };
}

/**
 * Advances the sticky note physics by `dtSec` seconds. Returns true once all
 * channels have settled cleanly so the animation loop can sleep at 0% CPU.
 */
export function stepStickyPhysics(
  state: StickyPhysicsState,
  targets: StickyPhysicsTargets,
  dtSec: number
): boolean {
  const dt = clamp(dtSec, 1 / 240, 0.04);

  if (targets.reduceMotion) {
    state.x = targets.x;
    state.y = targets.y;
    state.vx = 0;
    state.vy = 0;
    state.angle = state.restAngle;
    state.angleVel = 0;
    state.tiltX = 0;
    state.tiltY = 0;
    state.lift = targets.held ? 0.6 : 0;
    return !targets.held;
  }

  // Tight, responsive hand tracking while held; smooth settle when returning home.
  const prevX = state.x;
  const prevY = state.y;
  const prevVx = state.vx;
  const followRate = targets.held ? 30 : 18;
  const posAlpha = 1 - Math.exp(-dt * followRate);
  state.x += (targets.x - state.x) * posAlpha;
  state.y += (targets.y - state.y) * posAlpha;

  state.vx = (state.x - prevX) / dt;
  state.vy = (state.y - prevY) / dt;

  // Pendulum swing while carried in the air: horizontal acceleration kicks the
  // hanging bottom edge via inertia, while air drag trails it gently during travel.
  // Once released, it settles crisply flat onto the page with higher damping.
  const springK = targets.held ? 92 : 175;
  const damping = targets.held ? 7.8 : 22;

  let equilibrium = state.restAngle;
  let impulseAccel = 0;
  if (targets.held) {
    const ax = clamp((state.vx - prevVx) / dt, -14000, 14000);
    // Moving right (+vx) trails the bottom left (+deg in CSS clockwise rotation).
    equilibrium = clamp(state.vx * 0.011, -15, 15);
    impulseAccel = clamp(ax * 0.0085, -160, 160);
  }

  const angularAccel = (equilibrium - state.angle) * springK - state.angleVel * damping + impulseAccel;
  state.angleVel += angularAccel * dt;
  state.angle = clamp(state.angle + state.angleVel * dt, -MAX_SWING_DEG, MAX_SWING_DEG);

  const tiltAlpha = 1 - Math.exp(-dt * (targets.held ? 14 : 13));
  const liftAlpha = 1 - Math.exp(-dt * (targets.held ? 18 : 16));
  state.tiltX += (targets.tiltX - state.tiltX) * tiltAlpha;
  state.tiltY += (targets.tiltY - state.tiltY) * tiltAlpha;
  state.lift += (targets.lift - state.lift) * liftAlpha;

  return (
    !targets.held &&
    Math.abs(targets.x - state.x) + Math.abs(targets.y - state.y) < 0.08 &&
    Math.abs(state.restAngle - state.angle) < 0.04 &&
    Math.abs(state.angleVel) < 0.15 &&
    Math.abs(targets.tiltX - state.tiltX) +
      Math.abs(targets.tiltY - state.tiltY) +
      Math.abs(targets.lift - state.lift) <
      0.015
  );
}

export function attachStickyNote(note: HTMLElement, bounds: HTMLElement): void {
  const motion = window.matchMedia('(prefers-reduced-motion: reduce)');

  const state: StickyPhysicsState = {
    x: 0,
    y: 0,
    vx: 0,
    vy: 0,
    angle: DEFAULT_REST_ANGLE,
    angleVel: 0,
    restAngle: DEFAULT_REST_ANGLE,
    tiltX: 0,
    tiltY: 0,
    lift: 0,
  };

  const targets: StickyPhysicsTargets = {
    x: 0,
    y: 0,
    tiltX: 0,
    tiltY: 0,
    lift: 0,
    held: false,
    reduceMotion: motion.matches,
  };

  let frame = 0;
  let lastTime = 0;
  let grabOffsetX = 0;
  let grabOffsetY = 0;
  let limits = { minX: 0, maxX: 0, minY: 0, maxY: 0 };

  function render(): void {
    const scale = 1 + state.lift * 0.032;
    note.style.transform =
      `translate3d(${state.x.toFixed(2)}px, ${state.y.toFixed(2)}px, 0) ` +
      `scale(${scale.toFixed(4)}) ` +
      `perspective(900px) ` +
      `rotate(${state.angle.toFixed(2)}deg) ` +
      `rotateX(${state.tiltX.toFixed(2)}deg) ` +
      `rotateY(${state.tiltY.toFixed(2)}deg)`;
    note.style.setProperty('--lift', state.lift.toFixed(3));
  }

  function step(now: number): void {
    frame = 0;
    const dt = lastTime > 0 ? (now - lastTime) / 1000 : 1 / 60;
    lastTime = now;
    targets.reduceMotion = motion.matches;

    const settled = stepStickyPhysics(state, targets, dt);
    if (settled) {
      state.x = targets.x;
      state.y = targets.y;
      state.vx = 0;
      state.vy = 0;
      state.angle = state.restAngle;
      state.angleVel = 0;
      state.tiltX = targets.tiltX;
      state.tiltY = targets.tiltY;
      state.lift = targets.lift;
      lastTime = 0;
      render();
      return;
    }

    render();
    frame = window.requestAnimationFrame(step);
  }

  function wake(): void {
    if (frame === 0) {
      frame = window.requestAnimationFrame(step);
    }
  }

  // Measure bounds from the note's unrotated size around its current center so
  // 3D tilt or swing never skews the clamping area.
  function measureLimits(): void {
    const rect = note.getBoundingClientRect();
    const area = bounds.getBoundingClientRect();
    const halfW = note.offsetWidth * 0.5;
    const halfH = note.offsetHeight * 0.5;
    const centerX = rect.left + rect.width * 0.5;
    const centerY = rect.top + rect.height * 0.5;
    const rightEdge = Math.min(area.right, document.documentElement.clientWidth);

    limits = {
      minX: state.x + (Math.max(area.left, 0) + halfW - centerX),
      maxX: state.x + (rightEdge - halfW - centerX),
      minY: state.y + (area.top + halfH - centerY),
      maxY: state.y + (area.bottom - BOTTOM_INSET_PX - halfH - centerY),
    };
  }

  note.addEventListener('pointermove', (event) => {
    if (targets.held) {
      targets.x = clamp(event.pageX - grabOffsetX, limits.minX, limits.maxX);
      targets.y = clamp(event.pageY - grabOffsetY, limits.minY, limits.maxY);
      wake();
      return;
    }

    if (event.pointerType !== 'mouse' || motion.matches) return;
    const rect = note.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return;

    const peel = computeHoverPeel(
      (event.clientX - rect.left) / rect.width,
      (event.clientY - rect.top) / rect.height
    );
    targets.tiltX = peel.tiltX;
    targets.tiltY = peel.tiltY;
    targets.lift = peel.lift;
    wake();
  });

  note.addEventListener('pointerleave', () => {
    if (targets.held) return;
    targets.tiltX = 0;
    targets.tiltY = 0;
    targets.lift = 0;
    wake();
  });

  // Dragging is for mouse and pen; a finger on the note should scroll the page.
  note.addEventListener('pointerdown', (event) => {
    if (event.pointerType === 'touch' || event.button !== 0) return;
    event.preventDefault();
    note.setPointerCapture(event.pointerId);
    targets.held = true;
    note.toggleAttribute('data-held', true);
    grabOffsetX = event.pageX - state.x;
    grabOffsetY = event.pageY - state.y;
    measureLimits();
    // Slight gentle hang-back curl when lifted off the wall
    targets.tiltX = 3.5;
    targets.tiltY = 0;
    targets.lift = 1;
    wake();
  });

  function release(event: PointerEvent): void {
    if (!targets.held) return;
    targets.held = false;
    note.toggleAttribute('data-held', false);
    if (note.hasPointerCapture(event.pointerId)) note.releasePointerCapture(event.pointerId);

    // Keep a subtle natural resting angle influenced by the release swing, rather than a random jump.
    if (Math.abs(state.vx) > 25 || Math.abs(state.angle - state.restAngle) > 2) {
      const bias = state.vx > 25 ? 1.1 : state.vx < -25 ? -1.1 : 0;
      state.restAngle = clamp(state.angle * 0.35 + bias, -5.5, 5.5);
    }
    targets.tiltX = 0;
    targets.tiltY = 0;
    targets.lift = 0;
    wake();
  }

  note.addEventListener('pointerup', release);
  note.addEventListener('pointercancel', release);

  // Double-clicking a relocated note glides it smoothly back home.
  note.addEventListener('dblclick', () => {
    if (state.x === 0 && state.y === 0) return;
    targets.x = 0;
    targets.y = 0;
    state.restAngle = DEFAULT_REST_ANGLE;
    targets.tiltX = 0;
    targets.tiltY = 0;
    targets.lift = 0;
    wake();
  });

  window.addEventListener('resize', () => {
    if (targets.held || (state.x === 0 && state.y === 0)) return;
    measureLimits();
    const fits = limits.minX <= limits.maxX && limits.minY <= limits.maxY;
    targets.x = state.x = fits ? clamp(state.x, limits.minX, limits.maxX) : 0;
    targets.y = state.y = fits ? clamp(state.y, limits.minY, limits.maxY) : 0;
    render();
  });

  render();
}
