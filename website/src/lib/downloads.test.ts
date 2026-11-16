import { test } from "node:test";
import assert from "node:assert/strict";
import { parseTotalDownloads } from "./downloads";

test("the total is read from the package page markup", () => {
  const html = `<span class="d-block color-fg-muted text-small tmp-mb-1">Total downloads</span>\n            <h3 title="21287">21.3K</h3>`;
  assert.equal(parseTotalDownloads(html), 21287);
  assert.equal(parseTotalDownloads("<h3>Total downloads</h3>"), null);
});
