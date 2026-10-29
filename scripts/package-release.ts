import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { expandRuntimeArchive } from "../desktop/scripts/runtimeArchive";

const root = resolve(import.meta.dir, "..");
const version = (await readFile(join(root, "VERSION"), "utf8")).trim();
if (!/^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$/.test(version))
  throw new Error("Invalid VERSION");
if (process.platform !== "linux" || process.arch !== "x64")
  throw new Error("Release packaging requires Linux x86_64");
const expectedTag = process.env.RELEASE_TAG;
if (expectedTag && expectedTag !== `v${version}`)
  throw new Error("Release tag does not match VERSION");
const output = join(root, "dist/release");
await mkdir(output, { recursive: true });
const stage = await mkdtemp(join(output, ".stage-"));
async function run(
  command: [string, ...string[]],
  cwd = root,
  env = process.env,
): Promise<void> {
  const child = Bun.spawn(command, {
    cwd,
    env,
    stdout: "inherit",
    stderr: "inherit",
  });
  if ((await child.exited) !== 0) throw new Error(`${command[0]} failed`);
}
function git(args: string[]): string {
  const result = Bun.spawnSync(["git", ...args], { cwd: root });
  if (result.exitCode !== 0) throw new Error("Cannot read release source revision");
  return new TextDecoder().decode(result.stdout).trim();
}
const sha = git(["rev-parse", "HEAD"]);
const dirty = git(["status", "--porcelain"]).length > 0;
if (expectedTag && dirty)
  throw new Error("Official releases require a clean checkout");
const epoch = git(["show", "-s", "--format=%ct", "HEAD"]);
const assets: string[] = [];
async function archive(name: string): Promise<void> {
  const file = `${name}.tar.gz`;
  await run([
    "tar",
    "--sort=name",
    `--mtime=@${epoch}`,
    "--owner=0",
    "--group=0",
    "--numeric-owner",
    "-I",
    "gzip -n -6",
    "-cf",
    join(output, file),
    "-C",
    stage,
    name,
  ]);
  assets.push(file);
}
try {
  await run(["bun", "run", "build:desktop"]);
  const bundle = join(root, "desktop/build/stable-linux-x64/BoxBox");
  const metadata: unknown = JSON.parse(
    await readFile(join(bundle, "Resources/metadata.json"), "utf8"),
  );
  if (
    typeof metadata !== "object" ||
    metadata === null ||
    !("hash" in metadata) ||
    typeof metadata.hash !== "string" ||
    !/^[A-Za-z0-9_-]+$/.test(metadata.hash)
  )
    throw new Error("Invalid stable runtime metadata");
  const desktopName = `boxbox-desktop-v${version}-linux-amd64`;
  const desktopRoot = join(stage, desktopName);
  await mkdir(desktopRoot);
  const expanded = await expandRuntimeArchive(
    join(bundle, "Resources", `${metadata.hash}.tar.zst`),
    desktopRoot,
  );
  const runtime: unknown = JSON.parse(
    await readFile(join(expanded, "Resources/version.json"), "utf8"),
  );
  if (
    typeof runtime !== "object" ||
    runtime === null ||
    !("version" in runtime) ||
    runtime.version !== version ||
    !("channel" in runtime) ||
    runtime.channel !== "stable"
  )
    throw new Error(
      "Stable runtime version differs from VERSION; rebuild desktop",
    );
  const installer = await Bun.build({
    entrypoints: [join(root, "desktop/scripts/install.ts")],
    outdir: desktopRoot,
    naming: "install.js",
    target: "bun",
  });
  if (!installer.success)
    throw new AggregateError(installer.logs, "Could not bundle installer");
  await writeFile(
    join(desktopRoot, "install.sh"),
    `#!/bin/sh\nset -eu\nbase=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)\nexec "$base/BoxBox/bin/bun" "$base/install.js" --source "$base/BoxBox" "$@"\n`,
    { mode: 0o755 },
  );
  await cp(join(root, "LICENSE"), join(desktopRoot, "LICENSE"));
  await writeFile(
    join(desktopRoot, "README.txt"),
    `BoxBox Desktop v${version} — Linux x86_64\nSource: ${sha}\n\nExtract into a writable folder, then run ./install.sh (no sudo).\nGo, Bun and Docker are not required. GTK 3, WebKitGTK 4.1 and Ayatana AppIndicator are required.\nSee https://boxbox.radhey.dev/docs/desktop/ for dependencies, pairing, updates and removal.\nThe installer leaves your default file manager unchanged unless you pass --set-default.\n`,
  );
  await archive(desktopName);
  for (const arch of ["amd64", "arm64"]) {
    const name = `boxbox-v${version}-linux-${arch}`;
    const directory = join(stage, name);
    await mkdir(directory);
    await run(
      [
        "go",
        "build",
        "-trimpath",
        "-ldflags=-s -w",
        "-o",
        join(directory, "boxbox"),
        "./cmd/server",
      ],
      join(root, "backend"),
      { ...process.env, CGO_ENABLED: "0", GOOS: "linux", GOARCH: arch },
    );
    await cp(join(root, "backend/config.yaml"), join(directory, "config.yaml"));
    await cp(join(root, "LICENSE"), join(directory, "LICENSE"));
    await writeFile(
      join(directory, "README.txt"),
      `BoxBox server v${version} (${arch})\nSource: ${sha}\nConfigure credentials and mounts before starting.\nhttps://boxbox.radhey.dev/docs/quickstart/\nThis is the server binary, not the desktop app.\n`,
    );
    await archive(name);
  }
  const sums: string[] = [];
  for (const file of assets) {
    const hash = createHash("sha256");
    for await (const chunk of createReadStream(join(output, file)))
      hash.update(chunk);
    sums.push(`${hash.digest("hex")}  ${file}`);
  }
  await writeFile(join(output, "SHA256SUMS"), sums.join("\n") + "\n");
  await writeFile(
    join(output, "release-manifest.json"),
    JSON.stringify(
      {
        version,
        source: sha,
        dirty,
        desktop: ["linux-amd64"],
        server: ["linux-amd64", "linux-arm64"],
        assets,
      },
      null,
      2,
    ) + "\n",
  );
  console.log(`Release files: ${output}`);
} finally {
  await rm(stage, { recursive: true, force: true });
}
