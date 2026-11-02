import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmod,
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
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
      await mkdir(env.XDG_RUNTIME_DIR, { mode: 0o700 });
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
      await writeFile(join(resources, "version.json"), JSON.stringify({
        identifier: "io.boxbox.desktop", channel: "stable",
      }));
      const originalBuild = JSON.stringify({ chromiumFlags: { "disable-gpu": false } });
      await writeFile(join(resources, "build.json"), originalBuild, { mode: 0o444 });
      const preferences = join(root, "data/io.boxbox.desktop/stable/desktop");
      await mkdir(preferences, { recursive: true });
      await writeFile(join(preferences, "preferences.json"), JSON.stringify({ hardwareAcceleration: false }));
      await writeFile(join(bin, "electrobun-launcher"), `#!/bin/sh
set -eu
test "$1" = --portal
test "$BOXBOX_HARDWARE_ACCELERATION" = false
cat "$(dirname "$0")/../Resources/build.json"
`, { mode: 0o755 });
      // Exercise the installed service command after the registration runtime is gone.
      const activation = spawnSync(persistent, ["--portal"], { env, encoding: "utf8" });
      assert.equal(activation.status, 0, activation.stderr);
      assert.deepEqual(JSON.parse(activation.stdout), {
        chromiumFlags: { "disable-gpu": true, "disable-gpu-compositing": true },
      });
      assert.equal(await readFile(join(resources, "build.json"), "utf8"), originalBuild);
      assert.ok(!(await readdir(root)).some((name) => name.startsWith("boxbox-appimage.")));
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
}
