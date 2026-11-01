import { lstat, mkdir, readFile, writeFile, rename } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, isAbsolute, join } from "node:path";
import { quoteDesktopExecArgument } from "../src/desktop-entry";

const bus = "org.freedesktop.impl.portal.desktop.boxbox";
const iface = "org.freedesktop.impl.portal.FileChooser";
const marker = "# BoxBox file chooser override";
type Environment = Readonly<Record<string, string | undefined>>;

/** Changes only FileChooser in the effective configuration, with an explicit reversible opt-in. */
export function chooserRouting(source: string, enable: boolean): string {
  const lines = source.split("\n");
  const existing = lines.indexOf(marker);
  if (existing >= 0) {
    const old = lines[existing + 1];
    if (
      !old?.startsWith("# Previous: ") ||
      lines[existing + 2] !== `${iface}=boxbox`
    ) {
      throw new Error(
        "The managed chooser routing was edited; restore it manually before changing it",
      );
    }
    const previous = old.slice("# Previous: ".length);
    lines.splice(existing, 3, ...(previous ? [previous] : []));
  }
  if (!enable) return lines.join("\n");
  let group = false;
  let insertion = -1;
  let previous = "";
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i] ?? "";
    if (/^\s*\[/.test(line)) {
      if (group && insertion < 0) insertion = i;
      group = line.trim() === "[preferred]";
    }
    if (
      group &&
      new RegExp(`^\\s*${iface.replaceAll(".", "\\.")}\\s*=`).test(line)
    ) {
      if (previous) throw new Error("Duplicate FileChooser routing");
      previous = line;
      insertion = i;
    }
  }
  if (insertion < 0) {
    if (!lines.some((line) => line.trim() === "[preferred]"))
      lines.push("[preferred]");
    insertion = lines.length;
  }
  lines.splice(
    insertion,
    previous ? 1 : 0,
    marker,
    `# Previous: ${previous}`,
    `${iface}=boxbox`,
  );
  return lines.join("\n");
}
function location(env: Environment, key: string, fallback: string): string {
  const value = env[key] || fallback;
  if (!isAbsolute(value)) throw new Error(`${key} must be absolute`);
  return value;
}
async function safeRead(path: string): Promise<string | undefined> {
  try {
    const info = await lstat(path);
    if (!info.isFile() || info.size > 128 * 1024)
      throw new Error(`Refusing unsafe portal file: ${path}`);
    return await readFile(path, "utf8");
  } catch (error) {
    if (
      typeof error === "object" &&
      error !== null &&
      "code" in error &&
      error.code === "ENOENT"
    )
      return undefined;
    throw error;
  }
}
async function atomic(path: string, content: string) {
  await safeRead(path);
  await mkdir(dirname(path), { recursive: true });
  const temporary = `${path}.boxbox-${process.pid}`;
  await writeFile(temporary, content, { mode: 0o600, flag: "wx" });
  await rename(temporary, path);
}
export async function installChooserPortal(options: {
  launcher: string;
  enable: boolean;
  env?: Environment;
}) {
  const env = options.env ?? process.env;
  const home = env.HOME || homedir();
  const config = location(env, "XDG_CONFIG_HOME", join(home, ".config"));
  const data = location(env, "XDG_DATA_HOME", join(home, ".local/share"));
  const desktops = (env.XDG_CURRENT_DESKTOP || "")
    .split(":")
    .filter(Boolean)
    .map((name) => name.toLowerCase());
  if (desktops.some((name) => !/^[a-z0-9_-]+$/.test(name)))
    throw new Error("Invalid desktop name");
  const names = [
    ...desktops.map((name) => `${name}-portals.conf`),
    "portals.conf",
  ];
  const roots = [
    config,
    ...(env.XDG_CONFIG_DIRS || "/etc/xdg").split(":"),
    "/etc",
    data,
    ...(env.XDG_DATA_DIRS || "/usr/local/share:/usr/share").split(":"),
  ];
  let source = "";
  let sourcePath = "";
  for (const root of roots) {
    for (const name of names) {
      const path = join(root, "xdg-desktop-portal", name);
      const content = await safeRead(path);
      if (content !== undefined) {
        source = content;
        sourcePath = path;
        break;
      }
    }
    if (sourcePath) break;
  }
  if (!options.enable && !source.includes(marker)) {
    throw new Error(
      "BoxBox has no managed chooser override in the effective configuration. No defaults changed.",
    );
  }
  if (options.enable) {
    // Upstream discovers .portal metadata in its compiled-in directory unless explicitly overridden.
    const metadataRoot = location(
      env,
      "XDG_DESKTOP_PORTAL_DIR",
      "/usr/share/xdg-desktop-portal/portals",
    );
    if (!(await safeRead(join(metadataRoot, "boxbox.portal")))) {
      throw new Error(
        "Install boxbox.portal in the system portal metadata directory first. No defaults changed.",
      );
    }
    if (!isAbsolute(options.launcher))
      throw new Error("Portal launcher must be absolute");
    const service = join(data, "dbus-1/services", `${bus}.service`);
    const old = await safeRead(service);
    if (old !== undefined && !old.includes("# BoxBox managed"))
      throw new Error("Refusing to overwrite an unmanaged D-Bus service");
    await atomic(
      service,
      `# BoxBox managed\n[D-BUS Service]\nName=${bus}\nExec=${quoteDesktopExecArgument(options.launcher).replaceAll("%%", "%")} --portal\n`,
    );
  }
  const destination = sourcePath.startsWith(`${config}/`)
    ? sourcePath
    : join(config, "xdg-desktop-portal", names[0] || "portals.conf");
  await atomic(destination, chooserRouting(source, options.enable));
  return destination;
}
if (import.meta.main) {
  const args = process.argv.slice(2);
  const enable = args[0] === "--enable";
  if ((!enable && args[0] !== "--disable") || args.length > 2)
    throw new Error(
      "Usage: bun desktop/scripts/portal.ts --enable <installed-launcher> | --disable",
    );
  const path = await installChooserPortal({
    enable,
    launcher: args[1] || "/usr/bin/boxbox",
  });
  console.log(
    `Updated only FileChooser routing in ${path}. Restart the portal session or log in again to apply.`,
  );
}
