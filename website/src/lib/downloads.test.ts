import { test } from "node:test";
import assert from "node:assert/strict";
import { formatDownloads, parseDownloadsPayload, parseTotalDownloads } from "./downloads";

test("the total is read from the package page markup", () => {
  const html = `<span class="d-block color-fg-muted text-small tmp-mb-1">Total downloads</span>\n            <h3 title="21287">21.3K</h3>`;
  assert.equal(parseTotalDownloads(html), 21287);
  assert.equal(parseTotalDownloads("<h3>Total downloads</h3>"), null);
});

test("only a whole, non-negative total is accepted from the API", () => {
  assert.equal(parseDownloadsPayload({ total: 21287 }), 21287);
  for (const payload of [null, {}, { total: "21287" }, { total: -1 }, { total: 1.5 }, { error: "x" }]) {
    assert.equal(parseDownloadsPayload(payload), null);
  }
});

test("counts round down to what was really reached", () => {
  assert.equal(formatDownloads(950), "950");
  assert.equal(formatDownloads(21287), "21k");
  assert.equal(formatDownloads(999_999), "999k");
  assert.equal(formatDownloads(1_250_000), "1.2M");
});
