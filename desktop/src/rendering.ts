import { readFile, writeFile, rename, rm } from "node:fs/promises";
import { homedir } from "node:os";
import { isAbsolute, join } from "node:path";

export function hardwareAccelerationEnabled(): boolean {
  return process.env.BOXBOX_HARDWARE_ACCELERATION !== "false";
}

/** Reads the same app/channel preference file as Electrobun, without loading CEF. */
export async function readHardwareAcceleration(
  resources: string,
  env: NodeJS.ProcessEnv = process.env,
  home = homedir(),
): Promise<boolean> {
  const version: unknown = JSON.parse(await readFile(join(resources, "version.json"), "utf8"));
  if (typeof version !== "object" || version === null ||
      !("identifier" in version) || typeof version.identifier !== "string" ||
      !("channel" in version) || typeof version.channel !== "string" ||
      !/^[A-Za-z0-9][A-Za-z0-9_.-]*$/.test(version.identifier) || !/^[A-Za-z0-9][A-Za-z0-9_.-]*$/.test(version.channel)) {
    throw new Error("Invalid desktop version metadata");
  }
  const installedRoot = env.ELECTROBUN_INSTALL_ROOT_NAME;
  const channel = installedRoot && /^[A-Za-z0-9][A-Za-z0-9_.-]*$/.test(installedRoot) ? installedRoot : version.channel;
  const data = env.XDG_DATA_HOME && isAbsolute(env.XDG_DATA_HOME)
    ? env.XDG_DATA_HOME : join(home, ".local/share");
  try {
    const file = Bun.file(join(data, version.identifier, channel, "desktop/preferences.json"));
    if (!await file.exists()) return true;
    if (file.size > (128 << 10)) throw new Error("Desktop preferences exceed the size limit");
    const saved: unknown = await file.json();
    if (typeof saved !== "object" || saved === null) throw new Error("Invalid desktop preferences");
    return !("hardwareAcceleration" in saved) || saved.hardwareAcceleration !== false;
  } catch {
    console.warn("Could not read the rendering preference; using hardware acceleration");
    return true;
  }
}

/** The SDK reads build.json before creating CEF; apply the saved choice under the launch guard. */
export async function configureRendering(resources: string, env: NodeJS.ProcessEnv = process.env): Promise<boolean> {
  const enabled = await readHardwareAcceleration(resources, env);
  const path = join(resources, "build.json");
  const build: unknown = JSON.parse(await readFile(path, "utf8"));
  if (typeof build !== "object" || build === null || Array.isArray(build)) throw new Error("Invalid desktop build metadata");
  const flags = "chromiumFlags" in build ? build.chromiumFlags : {};
  if (typeof flags !== "object" || flags === null || Array.isArray(flags)) throw new Error("Invalid Chromium flags");
  if (!("disable-gpu" in flags) || flags["disable-gpu"] !== !enabled ||
      !("disable-gpu-compositing" in flags) || flags["disable-gpu-compositing"] !== !enabled) {
    const temporary = `${path}.rendering-${process.pid}`;
    try {
      await writeFile(temporary, JSON.stringify({ ...build, chromiumFlags: {
        ...flags, "disable-gpu": !enabled, "disable-gpu-compositing": !enabled,
      }}), { mode: 0o644 });
      await rename(temporary, path);
    } catch {
      await rm(temporary, { force: true }).catch(() => undefined);
      throw new Error("Cannot apply the rendering preference. Install BoxBox in a writable directory.");
    }
  }
  return enabled;
}
