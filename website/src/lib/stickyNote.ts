// A taped-up sheet of paper the visitor can play with. Under the pointer it
// lifts off the page; picked up, it trails behind the hand and swings from its
// tape; put down, it sticks wherever it lands.

const REST_ANGLE = 2.5;
const MAX_SWING = 24;

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

export function attachStickyNote(note: HTMLElement, bounds: HTMLElement): void {
  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  let x = 0;
  let y = 0;
  let targetX = 0;
  let targetY = 0;
  let angle = REST_ANGLE;
  let angleVelocity = 0;
  let restAngle = REST_ANGLE;
  let tiltX = 0;
  let tiltY = 0;
  let targetTiltX = 0;
  let targetTiltY = 0;
  let lift = 0;
  let targetLift = 0;
  let held = false;
  let frame = 0;
  let grabX = 0;
  let grabY = 0;
  let limits = { minX: 0, maxX: 0, minY: 0, maxY: 0 };

  function render() {
    note.style.transform = `translate3d(${x}px, ${y}px, 0) perspective(900px) rotate(${angle}deg) rotateX(${tiltX}deg) rotateY(${tiltY}deg)`;
    note.style.setProperty('--lift', lift.toFixed(3));
  }

  function step() {
    const previousX = x;
    const follow = reduceMotion ? 1 : 0.24;
    x += (targetX - x) * follow;
    y += (targetY - y) * follow;

    // Carried sideways, the loose end lags behind the tape; let go, it swings
    // back to rest as a damped spring.
    const angleGoal = held && !reduceMotion ? clamp((x - previousX) * 1.15, -MAX_SWING, MAX_SWING) : restAngle;
    angleVelocity = (angleVelocity + (angleGoal - angle) * 0.09) * 0.86;
    angle = reduceMotion ? restAngle : angle + angleVelocity;

    tiltX += (targetTiltX - tiltX) * 0.16;
    tiltY += (targetTiltY - tiltY) * 0.16;
    lift += (targetLift - lift) * 0.16;
    render();

    const settled =
      !held &&
      Math.abs(targetX - x) + Math.abs(targetY - y) < 0.05 &&
      Math.abs(angleGoal - angle) + Math.abs(angleVelocity) < 0.02 &&
      Math.abs(targetTiltX - tiltX) + Math.abs(targetTiltY - tiltY) + Math.abs(targetLift - lift) < 0.02;
    frame = settled ? 0 : window.requestAnimationFrame(step);
  }

  function wake() {
    if (frame === 0) frame = window.requestAnimationFrame(step);
  }

  // The sheet is fixed along its top edge, so it lifts most where the pointer
  // is furthest from the tape.
  function tiltToward(event: PointerEvent) {
    if (reduceMotion) return;
    const rect = note.getBoundingClientRect();
    const across = clamp((event.clientX - rect.left) / rect.width, 0, 1);
    const down = clamp((event.clientY - rect.top) / rect.height, 0, 1);
    targetTiltX = 4 + down * 12;
    targetTiltY = (0.5 - across) * 14;
  }

  note.addEventListener('pointermove', (event) => {
    if (held) {
      targetX = clamp(event.pageX - grabX, limits.minX, limits.maxX);
      targetY = clamp(event.pageY - grabY, limits.minY, limits.maxY);
    } else if (event.pointerType === 'mouse') {
      tiltToward(event);
      targetLift = 0.4;
    }
    wake();
  });

  note.addEventListener('pointerleave', () => {
    if (held) return;
    targetTiltX = 0;
    targetTiltY = 0;
    targetLift = 0;
    wake();
  });

  // Dragging is for mouse and pen; a finger on the note should scroll the page.
  note.addEventListener('pointerdown', (event) => {
    if (event.pointerType === 'touch' || event.button !== 0) return;
    event.preventDefault();
    note.setPointerCapture(event.pointerId);
    held = true;
    note.toggleAttribute('data-held', true);
    grabX = event.pageX - x;
    grabY = event.pageY - y;

    // How far the note may travel before any edge leaves its bounds.
    const rect = note.getBoundingClientRect();
    const area = bounds.getBoundingClientRect();
    const right = Math.min(area.right, document.documentElement.clientWidth);
    limits = {
      minX: x + Math.max(area.left, 0) - rect.left,
      maxX: x + right - rect.right,
      minY: y + area.top - rect.top,
      maxY: y + area.bottom - rect.bottom
    };
    targetLift = 1;
    wake();
  });

  function release(event: PointerEvent) {
    if (!held) return;
    held = false;
    note.toggleAttribute('data-held', false);
    if (note.hasPointerCapture(event.pointerId)) note.releasePointerCapture(event.pointerId);
    // Nobody sticks a note back perfectly straight.
    restAngle = (Math.random() - 0.5) * 9;
    targetLift = 0.4;
    wake();
  }

  note.addEventListener('pointerup', release);
  note.addEventListener('pointercancel', release);
  render();
}
