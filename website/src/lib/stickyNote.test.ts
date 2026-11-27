import { test } from "node:test";
import assert from "node:assert/strict";
import {
  computeHoverPeel,
  DEFAULT_REST_ANGLE,
  stepStickyPhysics,
  type StickyPhysicsState,
  type StickyPhysicsTargets,
} from "./stickyNote";

function makeState(): StickyPhysicsState {
  return {
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
}

test("computeHoverPeel lifts the hovered bottom corner toward the viewer and stays calm near the top tape", () => {
  const topTape = computeHoverPeel(0.5, 0);
  const bottomRight = computeHoverPeel(1, 1);
  const bottomLeft = computeHoverPeel(0, 1);

  assert.ok(bottomRight.tiltX > topTape.tiltX * 4);
  assert.ok(bottomRight.lift > topTape.lift * 2);
  assert.ok(bottomRight.tiltY < 0, "CSS negative rotateY lifts the right edge toward viewer");
  assert.ok(bottomLeft.tiltY > 0, "CSS positive rotateY lifts the left edge toward viewer");
});

test("stepStickyPhysics swings trailing behind sideways carry and settles crisply after release", () => {
  const state = makeState();
  const targets: StickyPhysicsTargets = {
    x: 160,
    y: 20,
    tiltX: 3.5,
    tiltY: 0,
    lift: 1,
    held: true,
    reduceMotion: false,
  };

  for (let i = 0; i < 8; i += 1) {
    stepStickyPhysics(state, targets, 1 / 60);
  }
  assert.ok(state.x > 100, "note should track pointer tightly while held");
  assert.ok(state.angle > DEFAULT_REST_ANGLE, "moving right should swing bottom left (+deg)");

  // Release and settle
  targets.held = false;
  targets.tiltX = 0;
  targets.tiltY = 0;
  targets.lift = 0;
  state.restAngle = 1.5;

  let settled = false;
  for (let i = 0; i < 120; i += 1) {
    settled = stepStickyPhysics(state, targets, 1 / 60);
    if (settled) break;
  }
  assert.equal(settled, true);
  assert.ok(Math.abs(state.angle - 1.5) < 0.05);
  assert.ok(state.lift < 0.02);
});

test("stepStickyPhysics behaves consistently across 60Hz and 120Hz frame steps", () => {
  const s60 = makeState();
  const s120 = makeState();
  const targets: StickyPhysicsTargets = {
    x: 120,
    y: 40,
    tiltX: 3.5,
    tiltY: 0,
    lift: 1,
    held: true,
    reduceMotion: false,
  };

  for (let i = 0; i < 12; i += 1) stepStickyPhysics(s60, targets, 1 / 60);
  for (let i = 0; i < 24; i += 1) stepStickyPhysics(s120, targets, 1 / 120);

  assert.ok(Math.abs(s60.x - s120.x) < 6, `x diverged: 60Hz=${s60.x} vs 120Hz=${s120.x}`);
  assert.ok(Math.abs(s60.lift - s120.lift) < 0.05);
});
