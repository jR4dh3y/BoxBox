import { test } from "node:test";
import assert from "node:assert/strict";
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

test("AppRun provides writable metadata, preserves arguments and exit code, and cleans up", async () => {
  const stage = await mkdtemp(join(tmpdir(), "boxbox-apprun-test-"));
  try {
    const appdir = join(stage, "App Dir");
    const temporary = join(stage, "temporary");
    await mkdir(join(appdir, "BoxBox/bin"), { recursive: true });
    await mkdir(join(appdir, "BoxBox/Resources"));
    await mkdir(temporary);
    const metadata = join(appdir, "BoxBox/Resources/build.json");
    await writeFile(metadata, "original", { mode: 0o444 });
    await copyFile(
      resolve(import.meta.dir, "../assets/AppRun"),
      join(appdir, "AppRun"),
    );
    await chmod(join(appdir, "AppRun"), 0o755);
    await writeFile(
      join(appdir, "BoxBox/bin/launcher"),
      `#!/bin/sh
set -eu
printf changed > "$(dirname "$0")/../Resources/build.json"
printf '%s\\n' "$@"
exit 23
`,
      { mode: 0o755 },
    );
    const command = join(stage, "boxbox");
    await symlink(join(appdir, "AppRun"), command);
    const child = Bun.spawn(
      [command, "--select", "/folder with spaces/a;$.txt"],
      {
        env: { ...process.env, TMPDIR: temporary },
        stdout: "pipe",
        stderr: "pipe",
      },
    );
    assert.equal(
      await child.exited,
      23,
      await new Response(child.stderr).text(),
    );
    assert.equal(
      await new Response(child.stdout).text(),
      "--select\n/folder with spaces/a;$.txt\n",
    );
    assert.equal(await readFile(metadata, "utf8"), "original");
    assert.deepEqual(await readdir(temporary), []);
  } finally {
    await rm(stage, { recursive: true, force: true });
  }
});
