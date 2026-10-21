import { readFileSync, readlinkSync, statSync } from "node:fs";
import { dirname, join, posix, resolve } from "node:path";

/**
 * A folder to open in the local BoxBox app.
 *
 * `path` and `select` are virtual mount paths. The desktop mounts `/` as
 * `root`, so the local path `/home/ana/Documents` becomes
 * `root/home/ana/Documents`.
 */
export interface LaunchTarget {
  readonly path: string;
  readonly select?: string;
}

export interface LaunchOptions {
  readonly cwd?: string;
}

interface ParsedArguments {
  readonly paths: readonly string[];
  readonly select?: string;
}

interface InspectedPath {
  readonly path: string;
  readonly directory: string;
  readonly isDirectory: boolean;
}

const URI_SCHEME = /^[A-Za-z][A-Za-z0-9+.-]*:/;

export function virtualPath(localPath: string): string {
  const normalized = posix.normalize(localPath);
  if (!normalized.startsWith("/")) {
    throw new Error("Launch paths must be absolute");
  }
  return normalized === "/" ? "root" : `root/${normalized.slice(1)}`;
}

/**
 * Accepts only a local path or an absolute file URL. HTTP(S), mount URLs and
 * other schemes are rejected, so launch arguments cannot select a protocol
 * handler or a remote resource.
 */
export function fileURLPath(value: string): string {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    throw new Error(`Invalid file URL: ${value}`);
  }
  if (url.protocol !== "file:") {
    throw new Error("Only local paths and file:// URLs can be opened");
  }
  if (
    url.username ||
    url.password ||
    (url.host !== "" && url.host.toLowerCase() !== "localhost")
  ) {
    throw new Error("Only local file URLs can be opened");
  }
  if (url.search || url.hash) {
    throw new Error("File URLs cannot have a query or fragment");
  }
  let decoded: string;
  try {
    decoded = decodeURIComponent(url.pathname);
  } catch {
    throw new Error(`Invalid percent encoding in file URL: ${value}`);
  }
  requirePathText(decoded, "File URL path");
  if (!decoded.startsWith("/")) {
    throw new Error("File URLs must contain an absolute path");
  }
  return posix.normalize(decoded);
}

/**
 * Parses the desktop launcher arguments.
 *
 * `boxbox /a/folder` browses `/a/folder`.
 * `boxbox /a/file` browses `/a` and selects the file.
 * `boxbox --select /a/file` does the same without opening the file.
 * A directory and `--select` may be combined when the file is directly in it.
 */
export function parseLaunchTarget(
  args: readonly string[],
  options: LaunchOptions = {},
): LaunchTarget | undefined {
  const parsed = parseArguments(args);
  const cwd = options.cwd ?? process.cwd();
  // File managers can pass several selected URLs; this app window opens the
  // first target and leaves the rest for a future tab implementation.
  const primaryArgument = parsed.paths[0];
  const primary =
    primaryArgument === undefined
      ? undefined
      : inspectPath(localPath(primaryArgument, cwd));

  if (parsed.select !== undefined) {
    const selected = inspectPath(localPath(parsed.select, cwd));
    if (selected.isDirectory) {
      throw new Error("--select expects a file, not a folder");
    }
    const directory = primary?.directory ?? selected.directory;
    if (dirname(selected.path) !== directory) {
      throw new Error("The selected file must be directly inside the requested folder");
    }
    return targetFor(directory, selected.path);
  }

  if (primary === undefined) return undefined;
  if (primary.isDirectory) return { path: virtualPath(primary.path) };
  return targetFor(primary.directory, primary.path);
}

/** Builds the URL the Svelte app reads for its initial folder and selection. */
export function browseURL(origin: string, target: LaunchTarget): string {
  const base = new URL(origin);
  if (
    (base.protocol !== "http:" && base.protocol !== "https:") ||
    base.username ||
    base.password
  ) {
    throw new Error("The browser origin must be an HTTP(S) URL without credentials");
  }
  const url = new URL("/browse", base.origin);
  url.searchParams.set("path", target.path);
  if (target.select !== undefined) {
    url.searchParams.set("select", target.select);
  }
  return url.href;
}

/** Convenience for the main process: parse argv and build its first URL. */
export function launchURL(
  args: readonly string[],
  origin: string,
  options: LaunchOptions = {},
): string | undefined {
  const target = parseLaunchTarget(args, options);
  return target === undefined ? undefined : browseURL(origin, target);
}

/**
 * One entry from the Linux process table, used to find the launcher that
 * started the packaged app.
 */
export interface ProcessEntry {
  readonly parentPid: number;
  readonly executable?: string;
  readonly arguments: readonly string[];
}

/**
 * Returns the user arguments for this launch.
 *
 * Electrobun's Linux launcher runs the app through `bun Resources/main.js`
 * without forwarding its own arguments, so a packaged app sees only the Bun
 * command line. The original arguments are still on the launcher process,
 * which waits for its child, so walk the ancestors and match the launcher
 * executable next to `process.argv0`.
 */
export function launcherArguments(isPackaged: boolean): readonly string[] {
  const direct = process.argv.slice(2);
  if (!isPackaged || process.platform !== "linux") {
    return direct;
  }
  const launcher = join(dirname(process.argv0), "launcher");
  const recovered = findLauncherArguments(
    join(dirname(process.argv0), "electrobun-launcher"), process.ppid, readProcess,
  ) ?? findLauncherArguments(launcher, process.ppid, readProcess);
  return recovered === undefined || recovered.length === 0 ? direct : recovered;
}

/**
 * Finds the nearest ancestor process that is `launcher` and returns its
 * arguments without the program name. Bounded so a malformed process tree
 * cannot loop or scan indefinitely.
 */
export function findLauncherArguments(
  launcher: string,
  startPid: number,
  inspect: (pid: number) => ProcessEntry | undefined,
): readonly string[] | undefined {
  const expected = resolve(launcher);
  const visited = new Set<number>();
  let pid = startPid;
  while (pid > 1 && visited.size < 6 && !visited.has(pid)) {
    visited.add(pid);
    const entry = inspect(pid);
    if (entry === undefined) return undefined;
    if (isLauncher(entry, expected)) return entry.arguments.slice(1);
    pid = entry.parentPid;
  }
  return undefined;
}

/** Splits a NUL-separated Linux command line and drops the launcher binary. */
export function commandLineArguments(raw: string): readonly string[] {
  return splitCommandLine(raw).slice(1);
}

function splitCommandLine(raw: string): readonly string[] {
  const args = raw.split("\0");
  if (args.at(-1) === "") args.pop();
  return args;
}

function isLauncher(entry: ProcessEntry, launcher: string): boolean {
  if (entry.executable !== undefined && resolve(entry.executable) === launcher) {
    return true;
  }
  const program = entry.arguments[0];
  return program !== undefined && resolve(program) === launcher;
}

function readProcess(pid: number): ProcessEntry | undefined {
  try {
    const stat = readFileSync(`/proc/${pid}/stat`, "utf8");
    const commandEnd = stat.lastIndexOf(")");
    if (commandEnd < 0) return undefined;
    const parentPid = Number(stat.slice(commandEnd + 2).split(" ")[1]);
    if (!Number.isSafeInteger(parentPid) || parentPid < 0) return undefined;
    const args = splitCommandLine(readFileSync(`/proc/${pid}/cmdline`, "utf8"));
    let executable: string | undefined;
    try {
      executable = readlinkSync(`/proc/${pid}/exe`);
    } catch {
      executable = undefined;
    }
    return { parentPid, executable, arguments: args };
  } catch {
    return undefined;
  }
}

function parseArguments(args: readonly string[]): ParsedArguments {
  const paths: string[] = [];
  let select: string | undefined;
  let endOfOptions = false;

  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === undefined) {
      throw new Error("Launch arguments cannot contain empty slots");
    }
    if (!endOfOptions && argument === "--") {
      endOfOptions = true;
      continue;
    }
    if (!endOfOptions && argument === "--select") {
      if (select !== undefined) {
        throw new Error("--select can be used only once");
      }
      const value = args[index + 1];
      if (value === undefined) {
        throw new Error("--select requires a file path");
      }
      select = requirePathText(value, "--select");
      index += 1;
      continue;
    }
    if (!endOfOptions && argument.startsWith("--select=")) {
      if (select !== undefined) {
        throw new Error("--select can be used only once");
      }
      select = requirePathText(argument.slice("--select=".length), "--select");
      continue;
    }
    if (!endOfOptions && argument.startsWith("-")) {
      throw new Error(`Unsupported launch option: ${argument}`);
    }
    paths.push(requirePathText(argument, "Launch path"));
  }

  return { paths, select };
}

function localPath(value: string, cwd: string): string {
  if (/^file:/i.test(value)) return fileURLPath(value);
  if (URI_SCHEME.test(value)) {
    throw new Error("Only local paths and file:// URLs can be opened");
  }
  const absolute = resolve(cwd, value);
  requirePathText(absolute, "Launch path");
  return absolute;
}

function inspectPath(path: string): InspectedPath {
  let info;
  try {
    info = statSync(path);
  } catch {
    throw new Error(`Cannot open ${path}: no such file or directory`);
  }
  if (info.isDirectory()) {
    return { path, directory: path, isDirectory: true };
  }
  if (info.isFile()) {
    return { path, directory: dirname(path), isDirectory: false };
  }
  throw new Error(`Only folders and regular files can be opened: ${path}`);
}

function targetFor(directory: string, select: string): LaunchTarget {
  return { path: virtualPath(directory), select: virtualPath(select) };
}

function requirePathText(value: string, label: string): string {
  if (value.length === 0) {
    throw new Error(`${label} cannot be empty`);
  }
  if (value.includes("\0") || value.includes("\n") || value.includes("\r")) {
    throw new Error(`${label} contains unsupported control characters`);
  }
  return value;
}
