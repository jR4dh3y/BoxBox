import { test } from "node:test";
import assert from "node:assert/strict";
import {
  bayerThreshold,
  ditherDotState,
  dotProximity,
  mixRgb,
  parseCssColor,
} from "./footerHalftone";

test("parseCssColor parses 3-digit hex, 6-digit hex, rgb strings, and falls back safely", () => {
  assert.deepEqual(parseCssColor("#4a9eff"), { r: 74, g: 158, b: 255 });
  assert.deepEqual(parseCssColor("#fff"), { r: 255, g: 255, b: 255 });
  assert.deepEqual(parseCssColor("rgb(232, 195, 106)"), { r: 232, g: 195, b: 106 });
  assert.deepEqual(parseCssColor("not-a-color"), { r: 74, g: 158, b: 255 });
});

test("mixRgb interpolates channels and clamps t", () => {
  assert.deepEqual(mixRgb({ r: 0, g: 100, b: 200 }, { r: 100, g: 200, b: 0 }, 0.5), {
    r: 50,
    g: 150,
    b: 100,
  });
  assert.deepEqual(mixRgb({ r: 10, g: 20, b: 30 }, { r: 40, g: 50, b: 60 }, 2), {
    r: 40,
    g: 50,
    b: 60,
  });
});

test("dotProximity drops quadratically from 1 at center to 0 at radius", () => {
  assert.equal(dotProximity(0, 100), 1);
  assert.equal(dotProximity(50, 100), 0.25);
  assert.equal(dotProximity(100, 100), 0);
  assert.equal(dotProximity(140, 100), 0);
});

test("bayerThreshold covers all 16 distinct thresholds in (0, 1) and tiles every 4 cells", () => {
  const seen = new Set<number>();
  for (let row = 0; row < 4; row += 1) {
    for (let col = 0; col < 4; col += 1) {
      const value = bayerThreshold(col, row);
      assert.ok(value > 0 && value < 1);
      seen.add(value);
      assert.equal(bayerThreshold(col + 4, row + 8), value);
    }
  }
  assert.equal(seen.size, 16);
});

test("ditherDotState transitions cleanly from rest to lit to off across its Bayer threshold", () => {
  assert.equal(ditherDotState(0, 0.5), "rest");
  assert.equal(ditherDotState(0.03, 0.5), "rest");
  assert.equal(ditherDotState(0.25, 0.5), "lit");
  assert.equal(ditherDotState(0.5, 0.5), "off");
  assert.equal(ditherDotState(0.95, 0.5), "off");
});
