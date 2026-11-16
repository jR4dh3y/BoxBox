import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";

if (process.argv.length > 2) throw new Error("Usage: bun scripts/package-release.ts (always builds fresh output)");

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
  const desktopName = `boxbox-desktop-v${version}-linux-amd64`;
  const desktopRoot = join(stage, desktopName);
  await mkdir(desktopRoot);
  await cp(
    join(root, "desktop/target/release/boxbox-gpui"),
    join(desktopRoot, "boxbox-files"),
  );
  for (const file of ["install.sh", "io.boxbox.files.desktop"])
    await cp(join(root, "desktop/packaging", file), join(desktopRoot, file));
  await cp(
    join(root, "frontend/static/logo-boxbox-3d.svg"),
    join(desktopRoot, "io.boxbox.files.svg"),
  );
  await cp(join(root, "LICENSE"), join(desktopRoot, "LICENSE"));
  await writeFile(
    join(desktopRoot, "README.txt"),
    `BoxBox Files v${version} — Linux x86_64\nSource: ${sha}\n\nExtract it anywhere, then run ./install.sh (no sudo). It installs to ~/.local.\nThe host needs Vulkan, xkbcommon, Wayland or X11, fontconfig and freetype.\nSee https://boxbox.radhey.dev/docs/desktop/ for dependencies, servers, updates and removal.\nThe installer leaves your default file manager unchanged unless you pass --set-default.\n`,
  );
  await archive(desktopName);
  // Stages the current frontend into the server's go:embed directory.
  await run(["bash", "scripts/build-single.sh"]);
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
