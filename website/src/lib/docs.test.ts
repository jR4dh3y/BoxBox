import { test } from "node:test";
import assert from "node:assert/strict";
import { getAdjacentDocs, getDocNav, getDocPage, getDocSections } from "./docs";

test("every docs page appears in exactly one sidebar section", () => {
  const grouped = getDocSections().flatMap((section) => section.items.map((item) => item.slug));
  const pages = getDocNav().map((doc) => doc.slug);
  assert.deepEqual([...grouped].sort(), [...pages].sort());
  assert.equal(new Set(grouped).size, grouped.length);
});

test("getAdjacentDocs follows sidebar order and returns null at the boundaries", () => {
  const sections = getDocSections();
  const ordered = sections.flatMap((section) => section.items);
  assert.ok(ordered.length >= 3);

  const first = getAdjacentDocs(ordered[0]!.slug, sections);
  assert.equal(first.prev, null);
  assert.equal(first.next?.slug, ordered[1]!.slug);

  const second = getAdjacentDocs(ordered[1]!.slug, sections);
  assert.equal(second.prev?.slug, ordered[0]!.slug);
  assert.equal(second.next?.slug, ordered[2]!.slug);

  const last = getAdjacentDocs(ordered[ordered.length - 1]!.slug, sections);
  assert.equal(last.prev?.slug, ordered[ordered.length - 2]!.slug);
  assert.equal(last.next, null);
});

test("desktop docs metadata covers both Windows and Linux", () => {
  const desktop = getDocNav().find((doc) => doc.slug === "desktop");
  assert.equal(desktop?.title, "Desktop App");
  assert.match(desktop?.description ?? "", /Windows and Linux/);
});

test("the release notes link opens the full v0.4.0 notes", async () => {
  const index = await getDocPage("release-notes");
  const path = index.html.match(/href="(\/docs\/[^"]+\/)">v0\.4\.0 notes<\/a>/)?.[1];
  assert.equal(path, "/docs/v0.4.0/");
  assert.ok(getDocNav().some(({ slug }) => `/docs/${slug}/` === path));
  const release = await getDocPage("v0.4.0");
  assert.ok(release.headings.some(({ id }) => id === "desktop-app"));
});
