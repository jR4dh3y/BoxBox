import { test } from "node:test";
import assert from "node:assert/strict";
import { getDocNav, getDocPage, getDocSections } from "./docs";

test("every docs page appears in exactly one sidebar section", () => {
  const grouped = getDocSections().flatMap((section) => section.items.map((item) => item.slug));
  const pages = getDocNav().map((doc) => doc.slug);
  assert.deepEqual([...grouped].sort(), [...pages].sort());
  assert.equal(new Set(grouped).size, grouped.length);
});

test("the release notes link opens the full v0.4.0 notes", async () => {
  const index = await getDocPage("release-notes");
  const path = index.html.match(/href="(\/docs\/[^"]+\/)">v0\.4\.0 notes<\/a>/)?.[1];
  assert.equal(path, "/docs/v0.4.0/");
  assert.ok(getDocNav().some(({ slug }) => `/docs/${slug}/` === path));
  const release = await getDocPage("v0.4.0");
  assert.ok(release.headings.some(({ id }) => id === "upgrading"));
});
