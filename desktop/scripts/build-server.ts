import { chmod, copyFile, mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { buildNativeThumbnails } from "./build-native";

const root = resolve(import.meta.dir, "../..");
if (process.platform !== "linux")
  throw new Error("BoxBox Desktop currently targets Linux");
const build = Bun.spawn(["bash", "scripts/build-single.sh"], {
  cwd: root,
  stdout: "inherit",
  stderr: "inherit",
});
if ((await build.exited) !== 0) throw new Error("BoxBox server build failed");
await buildNativeThumbnails();

const resources = resolve(root, "desktop/resources");
await mkdir(resources, { recursive: true });
await copyFile(
  resolve(root, "dist/boxbox"),
  resolve(resources, "boxbox-server"),
);
await chmod(resolve(resources, "boxbox-server"), 0o755);
