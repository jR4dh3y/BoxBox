import { cp, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { createHash } from "node:crypto";

// The release tag's source; the packaging tools may come from a newer revision.
const root = resolve(process.env.BOXBOX_SOURCE_ROOT ?? join(import.meta.dir, ".."));
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
const runtimeFile = process.env.APPIMAGE_RUNTIME;
if (!runtimeFile)
  throw new Error("APPIMAGE_RUNTIME must name the verified type2 runtime");
const binary = join(root, "desktop/target/release/boxbox-gpui");
const header = Buffer.from(await Bun.file(binary).slice(0, 64).arrayBuffer());
if (
  header.subarray(0, 4).toString() !== "\x7fELF" ||
  header.readUInt16LE(18) !== (process.arch === "x64" ? 62 : 183)
)
  throw new Error("Desktop binary is missing or for another architecture");
const output = join(root, "dist/appimages");
await mkdir(output, { recursive: true });
const stage = await mkdtemp(join(output, ".stage-"));
try {
  const appdir = join(stage, "BoxBox.AppDir");
  await mkdir(join(appdir, "usr/bin"), { recursive: true });
  await cp(binary, join(appdir, "usr/bin/boxbox-files"));
  await symlink("usr/bin/boxbox-files", join(appdir, "AppRun"));
  const entry = await readFile(
    join(root, "desktop/packaging/io.boxbox.files.desktop"),
    "utf8",
  );
  await writeFile(
    join(appdir, "io.boxbox.files.desktop"),
    entry.replace("@EXEC@", "boxbox-files"),
  );
  const icon = Bun.spawn(
    [
      "rsvg-convert",
      "--width",
      "256",
      "--height",
      "256",
      "--output",
      join(appdir, "io.boxbox.files.png"),
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
      "--runtime-file",
      resolve(runtimeFile),
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
