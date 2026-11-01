import { test } from "node:test";
import assert from "node:assert/strict";
import { chooserRouting } from "./portal";

test("chooser opt-in preserves other interfaces and restores the old route", () => {
  const original =
    "[preferred]\ndefault=gtk;wlr\norg.freedesktop.impl.portal.ScreenCast=wlr\norg.freedesktop.impl.portal.FileChooser=gtk\n# keep me\n";
  const enabled = chooserRouting(original, true);
  assert.ok(enabled.includes("default=gtk;wlr"));
  assert.ok(enabled.includes("org.freedesktop.impl.portal.ScreenCast=wlr"));
  assert.equal(chooserRouting(enabled, true), enabled);
  assert.equal(chooserRouting(enabled, false), original);
});
test("chooser opt-in appends to preferred group and refuses a changed managed line", () => {
  const original = "[preferred]\ndefault=gtk\n[other]\nkey=value\n";
  const enabled = chooserRouting(original, true);
  assert.ok(enabled.indexOf("FileChooser=boxbox") < enabled.indexOf("[other]"));
  assert.equal(chooserRouting(enabled, false), original);
  assert.throws(() =>
    chooserRouting(
      enabled.replace("FileChooser=boxbox", "FileChooser=kde"),
      false,
    ),
  );
});
