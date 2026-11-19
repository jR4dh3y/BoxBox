import { test } from "node:test";
import assert from "node:assert/strict";
import { formatCount, parseCountPayload } from "./counts";
import { parseStars } from "./stars";

test("only a whole, non-negative total is accepted from an endpoint", () => {
  assert.equal(parseCountPayload({ total: 21287 }), 21287);
  for (const payload of [null, {}, { total: "21287" }, { total: -1 }, { total: 1.5 }, { error: "x" }]) {
    assert.equal(parseCountPayload(payload), null);
  }
});

test("counts round down to what was really reached", () => {
  assert.equal(formatCount(950), "950");
  assert.equal(formatCount(21287), "21k");
  assert.equal(formatCount(999_999), "999k");
  assert.equal(formatCount(1_250_000), "1.2M");
});

test("the star count is read from the repository payload", () => {
  assert.equal(parseStars({ stargazers_count: 312, forks_count: 9 }), 312);
  for (const payload of [null, {}, { stargazers_count: "312" }, { stargazers_count: -1 }]) {
    assert.equal(parseStars(payload), null);
  }
});
