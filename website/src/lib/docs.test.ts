import { test } from "node:test";
import assert from "node:assert/strict";
import { getDocNav, getDocSections } from "./docs";

test("every docs page appears in exactly one sidebar section", () => {
  const grouped = getDocSections().flatMap((section) => section.items.map((item) => item.slug));
  const pages = getDocNav().map((doc) => doc.slug);
  assert.deepEqual([...grouped].sort(), [...pages].sort());
  assert.equal(new Set(grouped).size, grouped.length);
});
