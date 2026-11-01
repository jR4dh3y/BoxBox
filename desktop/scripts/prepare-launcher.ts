import { existsSync } from "node:fs";
import { chmod, readdir, readFile, rename, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";

export async function prepareLauncher(bundle: string): Promise<void> {
  const resources = join(bundle, "Resources");
  const bin = join(bundle, "bin");
  const launcher = join(bin, "launcher");
  const result = await Bun.build({
    entrypoints: [resolve(import.meta.dir, "../src/launcher.ts")],
    outdir: resources,
    naming: "boxbox-launcher.js",
    target: "bun",
  });
  if (!result.success) throw new AggregateError(result.logs, "Could not build desktop launcher");
  const current = await readFile(launcher);
  if (!current.subarray(0, 100).toString().includes("BoxBox pre-native launcher")) {
    await rename(launcher, join(bin, "electrobun-launcher"));
  }
  await writeFile(launcher, `#!/bin/sh
# BoxBox pre-native launcher
boxbox_bin=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
exec "$boxbox_bin/bun" "$boxbox_bin/../Resources/boxbox-launcher.js" "$@"
`, { mode: 0o755 });
  await chmod(launcher, 0o755);
}

// Hutch imports hooks, so import.meta.main is false even when this hook runs.
{
  if (process.env.ELECTROBUN_OS !== "linux") throw new Error("BoxBox currently targets Linux");
  const directory = process.env.ELECTROBUN_BUILD_DIR;
  if (!directory) throw new Error("Missing Electrobun build directory");
  const root = resolve(directory);
  const candidates = [root, ...(await readdir(root, { withFileTypes: true }))
    .filter((entry) => entry.isDirectory())
    .map((entry) => join(root, entry.name))];
  const bundle = candidates.find((candidate) =>
    existsSync(join(candidate, "bin/launcher")) &&
    existsSync(join(candidate, "Resources/app/boxbox-server"))
  );
  if (!bundle) throw new Error(`No desktop runtime found under ${root}`);
  await prepareLauncher(bundle);
}
