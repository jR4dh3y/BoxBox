import { test } from "node:test";
import assert from "node:assert/strict";
import {
  clamp,
  computeCurtainReveal,
  computeScrollReveal,
  shouldUseStickyCurtain,
} from "./footerParallax";

test("clamp bounds values inclusively", () => {
  assert.equal(clamp(-5, 0, 1), 0);
  assert.equal(clamp(0.4, 0, 1), 0.4);
  assert.equal(clamp(9, 0, 1), 1);
});

test("computeScrollReveal reaches 0 when footer rests at viewport bottom", () => {
  assert.equal(computeScrollReveal(200, 600, 800), 0);
  assert.equal(computeScrollReveal(120, 600, 800), 0);
});

test("computeScrollReveal eases from 1 on initial entry down to 0 at rest", () => {
  assert.equal(computeScrollReveal(800, 600, 800), 1);
  assert.equal(computeScrollReveal(500, 600, 800), 0.5);
});

test("computeScrollReveal returns 0 for degenerate measurements", () => {
  assert.equal(computeScrollReveal(400, 0, 800), 0);
  assert.equal(computeScrollReveal(400, 600, 0), 0);
});

test("computeCurtainReveal tracks uncover progress from 1 (covered) to 0 (fully revealed)", () => {
  // Curtain bottom at or below viewport bottom -> footer completely covered
  assert.equal(computeCurtainReveal(900, 700, 900), 1);
  assert.equal(computeCurtainReveal(1400, 700, 900), 1);
  // Curtain lifted halfway up the footer height -> 0.5
  assert.equal(computeCurtainReveal(550, 700, 900), 0.5);
  // Curtain lifted past full footer height -> 0 (resting)
  assert.equal(computeCurtainReveal(200, 700, 900), 0);
  assert.equal(computeCurtainReveal(50, 700, 900), 0);
});

test("shouldUseStickyCurtain only enables sticky reveal when footer fits and curtain fills the viewport on desktop", () => {
  assert.equal(shouldUseStickyCurtain(720, 900, 900, 1280), true);
  assert.equal(shouldUseStickyCurtain(720, 620, 900, 1280), false);
  assert.equal(shouldUseStickyCurtain(820, 1200, 700, 1280), false);
  assert.equal(shouldUseStickyCurtain(600, 900, 844, 390), false);
});
