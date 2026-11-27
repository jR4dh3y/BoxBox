import { test } from "node:test";
import assert from "node:assert/strict";
import {
  ambientValleyMotes,
  farRackLeds,
  foregroundFrameLeds,
  midgroundBayLeds,
  type SceneLed,
} from "./footerSceneLeds";

function assertLedsInBounds(leds: readonly SceneLed[], width: number, height: number): void {
  assert.ok(leds.length > 0);
  for (const led of leds) {
    assert.ok(led.x >= 0 && led.x <= width, `LED x=${led.x} out of 0..${width}`);
    assert.ok(led.y >= 0 && led.y <= height, `LED y=${led.y} out of 0..${height}`);
    assert.ok(led.r > 0 && led.r <= 6);
    assert.ok(led.durationSec > 1);
  }
}

test("all scene hardware LEDs lie strictly within their parent asset viewBox", () => {
  assertLedsInBounds(farRackLeds, 1600, 549);
  assertLedsInBounds(midgroundBayLeds, 1600, 490);
  assertLedsInBounds(foregroundFrameLeds, 1600, 215);
});

test("ambient valley motes stay centered in the open valley with valid motion timings", () => {
  assert.ok(ambientValleyMotes.length >= 10);
  for (const mote of ambientValleyMotes) {
    assert.ok(mote.leftPct >= 20 && mote.leftPct <= 80);
    assert.ok(mote.bottomPct >= 5 && mote.bottomPct <= 40);
    assert.ok(mote.sizePx > 0 && mote.sizePx <= 4);
    assert.ok(mote.durationSec >= 8);
  }
});
