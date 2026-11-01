import { copyFile, mkdir } from "node:fs/promises";
import { resolve } from "node:path";

export async function buildNativeThumbnails(): Promise<void> {
  if (process.platform !== "linux")
    throw new Error("Native thumbnails currently target Linux");
  const root = resolve(import.meta.dir, "../..");
  const cargo = Bun.spawn(
    [
      "cargo",
      "build",
      "--release",
      "--locked",
      "--manifest-path",
      "desktop/native/Cargo.toml",
    ],
    {
      cwd: root,
      stdout: "inherit",
      stderr: "inherit",
    },
  );
  if ((await cargo.exited) !== 0)
    throw new Error("Rust thumbnail build failed");
  const resources = resolve(root, "desktop/resources/native");
  await mkdir(resources, { recursive: true });
  await copyFile(
    resolve(root, "desktop/native/target/release/libboxbox_thumbnail.so"),
    resolve(resources, "libboxbox_thumbnail.so"),
  );
  const result = await Bun.build({
    entrypoints: [resolve(root, "desktop/src/thumbnails/worker.ts")],
    outdir: resources,
    naming: "thumbnail-worker.js",
    target: "bun",
  });
  if (!result.success)
    throw new AggregateError(result.logs, "Thumbnail worker build failed");
}

if (import.meta.main) await buildNativeThumbnails();
