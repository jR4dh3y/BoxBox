import { test } from "node:test";
import assert from "node:assert/strict";
import {
  gridDotMode,
  introSweepEnergy,
  verticalGridFade,
} from "./interactiveDotGrid";

test("verticalGridFade fades linearly from 1 at top to 0 at bottom", () => {
  assert.equal(verticalGridFade(0, 900), 1);
  assert.equal(verticalGridFade(450, 900), 0.5);
  assert.equal(verticalGridFade(900, 900), 0);
  assert.equal(verticalGridFade(1200, 900), 0);
  assert.equal(verticalGridFade(100, 0), 0);
});

test("gridDotMode snaps dots cleanly between rest, whisper, and lit across Bayer thresholds", () => {
  assert.equal(gridDotMode(0, 0.5), "rest");
  assert.equal(gridDotMode(0.03, 0.5), "rest");
  assert.equal(gridDotMode(0.2, 0.5), "whisper");
  assert.equal(gridDotMode(0.4, 0.5), "lit");
  assert.equal(gridDotMode(0.95, 0.9), "lit");
});

test("introSweepEnergy sweeps across the grid on load and drops to 0 afterward", () => {
  assert.ok(introSweepEnergy(0.5, 0.4, 0.8) > 0.35);
  assert.equal(introSweepEnergy(0.5, 0.4, -0.1), 0);
  assert.equal(introSweepEnergy(0.5, 0.4, 2.5), 0);
});
