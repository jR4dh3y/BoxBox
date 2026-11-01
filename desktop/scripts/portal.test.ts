import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmod,
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
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

for (const appImage of [false, true]) {
  test(`AppRun registers a persistent chooser entry point, AppImage=${appImage}`, async () => {
    const root = await mkdtemp(join(tmpdir(), "boxbox-portal-apprun-"));
    try {
      const appdir = join(root, "AppDir");
      const bin = join(appdir, "BoxBox/bin");
      const resources = join(appdir, "BoxBox/Resources");
      await mkdir(bin, { recursive: true });
      await mkdir(resources, { recursive: true });
      await copyFile(
        resolve(import.meta.dir, "../assets/AppRun"),
        join(appdir, "AppRun"),
      );
      await chmod(join(appdir, "AppRun"), 0o755);
      await symlink(process.execPath, join(bin, "bun"));
      await writeFile(
        join(bin, "launcher"),
        '#!/bin/sh\nexec "$(dirname "$0")/bun" "$(dirname "$0")/../Resources/boxbox-launcher.js" "$@"\n',
        { mode: 0o755 },
      );
      const build = await Bun.build({
        entrypoints: [resolve(import.meta.dir, "../src/launcher.ts")],
        target: "bun",
        outdir: resources,
        naming: "boxbox-launcher.js",
      });
      assert.equal(build.success, true);
      const metadata = join(root, "metadata");
      await mkdir(metadata);
      await writeFile(
        join(metadata, "boxbox.portal"),
        "[portal]\nDBusName=org.freedesktop.impl.portal.desktop.boxbox\nInterfaces=org.freedesktop.impl.portal.FileChooser;\n",
      );
      const persistent = appImage
        ? join(root, "BoxBox.AppImage")
        : join(appdir, "AppRun");
      if (appImage) await symlink(join(appdir, "AppRun"), persistent);
      const env = {
        ...process.env,
        HOME: root,
        XDG_CONFIG_HOME: join(root, "config"),
        XDG_DATA_HOME: join(root, "data"),
        XDG_CONFIG_DIRS: join(root, "config-dirs"),
        XDG_DATA_DIRS: join(root, "data-dirs"),
        XDG_CURRENT_DESKTOP: "BoxBoxTest",
        XDG_DESKTOP_PORTAL_DIR: metadata,
        XDG_RUNTIME_DIR: join(root, "runtime"),
        APPIMAGE: appImage ? persistent : "",
        TMPDIR: root,
      };
      const run = spawnSync(join(appdir, "AppRun"), ["--enable-chooser"], {
        env,
        encoding: "utf8",
      });
      assert.equal(run.status, 0, run.stderr);
      const service = await readFile(
        join(
          root,
          "data/dbus-1/services/org.freedesktop.impl.portal.desktop.boxbox.service",
        ),
        "utf8",
      );
      assert.ok(service.includes(`Exec="${persistent}" --portal`), service);
      assert.ok(!service.includes("boxbox-appimage."), service);
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
}
