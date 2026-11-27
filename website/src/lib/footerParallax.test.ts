import { test } from "node:test";
import assert from "node:assert/strict";
import { clamp, computeScrollReveal } from "./footerParallax";

test("clamp bounds values inclusively", () => {
  assert.equal(clamp(-5, 0, 1), 0);
  assert.equal(clamp(0.42, 0, 1), 0.42);
  assert.equal(clamp(9, 0, 1), 1);
});

test("computeScrollReveal reaches 0 when footer rests at viewport bottom", () => {
  assert.equal(computeScrollReveal(200, 700, 900), 0);
  assert.equal(computeScrollReveal(120, 700, 900), 0);
});

test("computeScrollReveal eases from 1 on initial entry down to 0 at rest", () => {
  const entering = computeScrollReveal(900, 700, 900);
  const halfway = computeScrollReveal(550, 700, 900);
  const resting = computeScrollReveal(200, 700, 900);

  assert.equal(entering, 1);
  assert.equal(halfway, 0.5);
  assert.equal(resting, 0);
});

test("computeScrollReveal returns 0 for degenerate measurements", () => {
  assert.equal(computeScrollReveal(400, 0, 900), 0);
  assert.equal(computeScrollReveal(400, 700, 0), 0);
});
