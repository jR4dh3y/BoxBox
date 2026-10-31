import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { createHash } from "node:crypto";
import { expandRuntimeArchive } from "../desktop/scripts/runtimeArchive";

const tooling = resolve(import.meta.dir, "..");
const root = resolve(process.env.BOXBOX_SOURCE_ROOT ?? tooling);
if (
  process.platform !== "linux" ||
  (process.arch !== "x64" && process.arch !== "arm64")
)
  throw new Error("AppImages require a native Linux x64 or ARM64 builder");
const arch = process.arch === "x64" ? "x86_64" : "aarch64";
const version = (await readFile(join(root, "VERSION"), "utf8")).trim();
if (!/^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$/.test(version))
  throw new Error("Invalid VERSION");
if (process.env.RELEASE_TAG && process.env.RELEASE_TAG !== `v${version}`)
  throw new Error("Release tag does not match VERSION");
const tool = process.env.APPIMAGETOOL;
if (!tool)
  throw new Error(
    "APPIMAGETOOL must name the verified appimagetool executable",
  );
const output = join(root, "dist/appimages");
await mkdir(output, { recursive: true });
const stage = await mkdtemp(join(output, ".stage-"));
try {
  const appdir = join(stage, "BoxBox.AppDir");
  await mkdir(appdir);
  const bundle = join(
    root,
    `desktop/build/stable-linux-${process.arch}/BoxBox`,
  );
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
  const runtime = await expandRuntimeArchive(
    join(bundle, "Resources", `${metadata.hash}.tar.zst`),
    appdir,
  );
  const identity: unknown = JSON.parse(
    await readFile(join(runtime, "Resources/version.json"), "utf8"),
  );
  if (
    typeof identity !== "object" ||
    identity === null ||
    !("version" in identity) ||
    identity.version !== version ||
    !("channel" in identity) ||
    identity.channel !== "stable"
  )
    throw new Error("Stable runtime does not match the release version");
  for (const binary of [
    "bin/bun",
    "bin/electrobun-launcher",
    "Resources/app/boxbox-server",
  ]) {
    const header = Buffer.from(
      await Bun.file(join(runtime, binary)).slice(0, 64).arrayBuffer(),
    );
    if (
      header.subarray(0, 4).toString() !== "\x7fELF" ||
      header.readUInt16LE(18) !== (process.arch === "x64" ? 62 : 183)
    )
      throw new Error(`Wrong native architecture: ${binary}`);
  }
  await cp(join(tooling, "desktop/assets/AppRun"), join(appdir, "AppRun"));
  const template = await readFile(
    join(root, "desktop/assets/boxbox.desktop"),
    "utf8",
  );
  await writeFile(
    join(appdir, "io.boxbox.desktop"),
    template.replace("@EXEC@", "AppRun"),
  );
  const icon = Bun.spawn(
    [
      "rsvg-convert",
      "--width",
      "256",
      "--height",
      "256",
      "--output",
      join(appdir, "io.boxbox.desktop.png"),
      join(root, "frontend/static/logo-boxbox-3d.svg"),
    ],
    { stdout: "inherit", stderr: "inherit" },
  );
  if ((await icon.exited) !== 0)
    throw new Error("Could not render the AppImage icon");
  const file = `boxbox-desktop-v${version}-${arch}.AppImage`;
  const child = Bun.spawn(
    [
      resolve(tool),
      "--appimage-extract-and-run",
      "--no-appstream",
      appdir,
      join(output, file),
    ],
    {
      cwd: stage,
      env: { ...process.env, ARCH: arch },
      stdout: "inherit",
      stderr: "inherit",
    },
  );
  if ((await child.exited) !== 0) throw new Error("AppImage packaging failed");
  const hash = createHash("sha256");
  for await (const chunk of Bun.file(join(output, file)).stream())
    hash.update(chunk);
  await writeFile(
    join(output, `${file}.sha256`),
    `${hash.digest("hex")}  ${file}\n`,
  );
} finally {
  await rm(stage, { recursive: true, force: true });
}
