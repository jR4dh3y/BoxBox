import { quoteDesktopExecArgument } from "../src/desktop-entry";
export { quoteDesktopExecArgument } from "../src/desktop-entry";
import desktopTemplate from "../assets/boxbox.desktop" with { type: "text" };
import iconSource from "../../frontend/static/logo-boxbox-3d.svg" with { type: "text" };
import { spawnSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import {
  chmod,
  cp,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  rename,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { expandRuntimeArchive } from "./runtimeArchive";

const APP_ID = "io.boxbox.desktop";
const APP_NAME = "BoxBox";
const INSTALL_MARKER = ".boxbox-install.json";
const DESKTOP_MARKER = "X-BoxBox-Managed=true";
const DEFAULT_SOURCE = resolve(
  import.meta.dir,
  "../build/stable-linux-x64/BoxBox",
);

type Environment = Readonly<Record<string, string | undefined>>;

export type CommandRunner = (
  command: readonly string[],
  env: Readonly<Record<string, string>>,
) => void;

export interface InstallOptions {
  /** The unpacked stable bundle. Defaults to desktop/build/stable-linux-x64/BoxBox. */
  readonly source?: string;
  /** Overrides process.env; tests pass isolated XDG directories here. */
  readonly env?: Environment;
  /** Explicit opt-in to make BoxBox the default folder and file:// handler. */
  readonly setDefault?: boolean;
  readonly runCommand?: CommandRunner;
}

export interface InstallResult {
  readonly appDirectory: string;
  readonly launcher: string;
  readonly desktopFile: string;
  readonly iconFile: string;
}

export interface InstallArguments {
  readonly source?: string;
  readonly setDefault: boolean;
  readonly help: boolean;
}

/**
 * Installs a built stable bundle for the current user.
 *
 * Files live under the XDG data home, never under the user's file-manager
 * roots. An existing app directory is replaced only when it carries the
 * installer marker, so a mistaken path cannot delete unrelated data.
 */
export async function installDesktopIntegration(
  options: InstallOptions = {},
): Promise<InstallResult> {
  const env = fullEnvironment(options.env ?? process.env);
  const dataHome = xdgDataHome(env);
  const appDirectory = join(dataHome, "boxbox");
  const source = resolve(options.source ?? DEFAULT_SOURCE);

  if (containsPath(appDirectory, source) || containsPath(source, appDirectory)) {
    throw new Error(
      "The bundle source and install directory must not contain one another",
    );
  }

  const bundle = await inspectBundle(source);

  const applicationsDirectory = join(dataHome, "applications");
  // The Electrobun identifier already carries the .desktop suffix, so the
  // same string is both the desktop file ID and the installed file name.
  const desktopFile = join(applicationsDirectory, APP_ID);
  const iconDirectory = join(
    dataHome,
    "icons",
    "hicolor",
    "scalable",
    "apps",
  );
  const iconFile = join(iconDirectory, `${APP_ID}.svg`);
  const launcher = join(appDirectory, "bin", "launcher");

  const existingApp = await inspectAppDirectory(appDirectory);
  await inspectDesktopFile(desktopFile);
  await inspectWritableFile(iconFile);

  const desktopEntry = renderDesktopEntry(launcher);
  const icon = iconSource;

  await mkdir(dataHome, { recursive: true, mode: 0o755 });
  const stageRoot = await mkdtemp(join(dataHome, ".boxbox-stage-"));
  try {
    const stagedApp =
      bundle.kind === "expanded"
        ? await stageExpandedBundle(bundle.directory, stageRoot)
        : await stageArchiveBundle(bundle.archive, stageRoot);
    await writeFile(
      join(stagedApp, INSTALL_MARKER),
      `${JSON.stringify({ schema: 1, identifier: APP_ID })}\n`,
      { mode: 0o644 },
    );
    await replaceAppDirectory(
      stagedApp,
      appDirectory,
      dataHome,
      existingApp,
    );
  } finally {
    await rm(stageRoot, { recursive: true, force: true }).catch(() => {
      // The installed app is already in place; leaking only our staging
      // directory is safer than failing a completed install.
    });
  }

  await mkdir(applicationsDirectory, { recursive: true, mode: 0o755 });
  await mkdir(iconDirectory, { recursive: true, mode: 0o755 });
  await writeFileAtomic(iconFile, icon);
  await writeFileAtomic(desktopFile, desktopEntry);

  if (options.setDefault) {
    const runner = options.runCommand ?? runXdgMime;
    runner(
      [
        "xdg-mime",
        "default",
        APP_ID,
        "inode/directory",
        "x-scheme-handler/file",
      ],
      env,
    );
  }

  return { appDirectory, launcher, desktopFile, iconFile };
}

export function parseInstallArguments(
  args: readonly string[],
): InstallArguments {
  let source: string | undefined;
  let setDefault = false;
  let help = false;

  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === undefined) {
      throw new Error("Install arguments cannot contain empty slots");
    }
    if (argument === "--help" || argument === "-h") {
      help = true;
      continue;
    }
    if (argument === "--set-default") {
      setDefault = true;
      continue;
    }
    if (argument === "--source") {
      if (source !== undefined) {
        throw new Error("--source can be used only once");
      }
      const value = args[index + 1];
      if (value === undefined || value.length === 0) {
        throw new Error("--source requires a bundle directory");
      }
      source = value;
      index += 1;
      continue;
    }
    if (argument.startsWith("--source=")) {
      if (source !== undefined) {
        throw new Error("--source can be used only once");
      }
      const value = argument.slice("--source=".length);
      if (value.length === 0) {
        throw new Error("--source requires a bundle directory");
      }
      source = value;
      continue;
    }
    throw new Error(`Unknown install option: ${argument}`);
  }

  return { source, setDefault, help };
}

function fullEnvironment(env: Environment): Record<string, string> {
  const result: Record<string, string> = {};
  for (const [name, value] of Object.entries(env)) {
    if (value !== undefined) result[name] = value;
  }
  return result;
}

function xdgDataHome(env: Readonly<Record<string, string>>): string {
  const configured = env.XDG_DATA_HOME;
  if (configured !== undefined && configured.length > 0) {
    if (!isAbsolute(configured)) {
      throw new Error("XDG_DATA_HOME must be an absolute path");
    }
    return resolve(configured);
  }
  const home = env.HOME;
  if (home === undefined || home.length === 0 || !isAbsolute(home)) {
    throw new Error("HOME must be an absolute path");
  }
  return join(home, ".local", "share");
}

function containsPath(parent: string, child: string): boolean {
  const path = relative(parent, child);
  return (
    path === "" ||
    (path !== ".." && !path.startsWith(`..${sep}`) && !isAbsolute(path))
  );
}

type StableBundle =
  | { readonly kind: "expanded"; readonly directory: string }
  | { readonly kind: "archive"; readonly archive: string };

interface BundleMetadata {
  readonly identifier: string;
  readonly channel: string;
  readonly hash?: string;
}

async function inspectBundle(source: string): Promise<StableBundle> {
  const metadataPath = join(source, "Resources", "metadata.json");
  const metadata = await readMetadata(metadataPath);
  const runtimeServer = join(source, "Resources", "app", "boxbox-server");

  // An already-expanded runtime has no wrapper metadata; its version file is
  // the identity. The self-extracting wrapper keeps the hash in metadata.
  if (await isRegularFile(runtimeServer)) {
    if (metadata === undefined) {
      await validateVersionIdentity(join(source, "Resources", "version.json"));
    } else {
      requireStableMetadata(metadata);
    }
    await requireRuntimeFile(join(source, "bin", "launcher"));
    return { kind: "expanded", directory: source };
  }

  if (metadata === undefined) {
    throw new Error(
      `Not a stable BoxBox desktop bundle (missing ${metadataPath}). Run "bun run build:desktop" first.`,
    );
  }
  requireStableMetadata(metadata);

  // A self-extracting build ships the runtime as Resources/<hash>.tar.zst.
  // Installing its wrapper would make the wrapper install itself elsewhere.
  const hash = metadata.hash;
  if (hash === undefined || !/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(hash)) {
    throw new Error(
      `Not a stable BoxBox desktop bundle (missing runtime payload in ${source}). Run "bun run build:desktop" first.`,
    );
  }
  const archive = join(source, "Resources", `${hash}.tar.zst`);
  if (!(await isRegularFile(archive))) {
    throw new Error(
      `Not a stable BoxBox desktop bundle (missing ${archive}). Run "bun run build:desktop" first.`,
    );
  }
  return { kind: "archive", archive };
}

function requireStableMetadata(metadata: BundleMetadata): void {
  if (metadata.identifier !== APP_ID) {
    throw new Error(`Bundle metadata does not identify ${APP_NAME}`);
  }
  if (metadata.channel !== "stable") {
    throw new Error("Only stable desktop bundles can be installed");
  }
}

async function readMetadata(path: string): Promise<BundleMetadata | undefined> {
  let raw: string;
  try {
    raw = await readFile(path, "utf8");
  } catch (error) {
    if (isMissing(error)) return undefined;
    throw error;
  }
  let metadata: unknown;
  try {
    metadata = JSON.parse(raw);
  } catch {
    throw new Error(`Invalid bundle metadata: ${path}`);
  }
  if (!isRecord(metadata)) {
    throw new Error(`Invalid bundle metadata: ${path}`);
  }
  const identifier = metadata["identifier"];
  const channel = metadata["channel"];
  const hash = metadata["hash"];
  return {
    identifier: typeof identifier === "string" ? identifier : "",
    channel: typeof channel === "string" ? channel : "",
    hash: typeof hash === "string" ? hash : undefined,
  };
}

async function validateVersionIdentity(path: string): Promise<void> {
  let raw: string;
  try {
    raw = await readFile(path, "utf8");
  } catch {
    throw new Error(`The runtime bundle is missing ${path}`);
  }
  let version: unknown;
  try {
    version = JSON.parse(raw);
  } catch {
    throw new Error(`Invalid runtime version metadata: ${path}`);
  }
  if (
    !isRecord(version) ||
    version["identifier"] !== APP_ID ||
    version["channel"] !== "stable"
  ) {
    throw new Error(`The runtime bundle does not identify ${APP_NAME}: ${path}`);
  }
}

async function stageExpandedBundle(
  directory: string,
  stageRoot: string,
): Promise<string> {
  const stagedApp = join(stageRoot, "app");
  await cp(directory, stagedApp, {
    recursive: true,
    preserveTimestamps: true,
    verbatimSymlinks: true,
  });
  await requireRuntime(stagedApp, false);
  return stagedApp;
}

async function stageArchiveBundle(
  archive: string,
  stageRoot: string,
): Promise<string> {
  const stagedApp = await expandRuntimeArchive(archive, stageRoot);
  await requireRuntime(stagedApp, true);
  return stagedApp;
}

async function requireRuntime(
  directory: string,
  requireVersion: boolean,
): Promise<void> {
  await requireRuntimeFile(join(directory, "bin", "launcher"));
  const server = join(directory, "Resources", "app", "boxbox-server");
  let info;
  try {
    info = await lstat(server);
  } catch {
    throw new Error(`The runtime bundle is missing ${server}`);
  }
  if (!info.isFile()) {
    throw new Error(`The runtime bundle has no file at ${server}`);
  }
  if (requireVersion) {
    await validateVersionIdentity(join(directory, "Resources", "version.json"));
  }
}

async function requireRuntimeFile(path: string): Promise<void> {
  let info;
  try {
    info = await lstat(path);
  } catch {
    throw new Error(`Bundle launcher is missing: ${path}`);
  }
  if (!info.isFile() || (info.mode & 0o111) === 0) {
    throw new Error(`Bundle launcher is not an executable file: ${path}`);
  }
}

async function isRegularFile(path: string): Promise<boolean> {
  try {
    return (await lstat(path)).isFile();
  } catch (error) {
    if (isMissing(error)) return false;
    throw error;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

async function inspectAppDirectory(appDirectory: string): Promise<boolean> {
  let info;
  try {
    info = await lstat(appDirectory);
  } catch (error) {
    if (isMissing(error)) return false;
    throw error;
  }
  if (info.isSymbolicLink() || !info.isDirectory()) {
    throw new Error(
      `${appDirectory} exists and is not a BoxBox install directory; refusing to replace it`,
    );
  }
  let raw: string;
  try {
    raw = await readFile(join(appDirectory, INSTALL_MARKER), "utf8");
  } catch {
    throw new Error(
      `${appDirectory} was not installed by BoxBox; refusing to replace it`,
    );
  }
  let marker: unknown;
  try {
    marker = JSON.parse(raw);
  } catch {
    throw new Error(
      `${appDirectory} has an invalid BoxBox install marker; refusing to replace it`,
    );
  }
  if (!isManagedInstall(marker)) {
    throw new Error(
      `${appDirectory} has an invalid BoxBox install marker; refusing to replace it`,
    );
  }
  return true;
}

async function inspectDesktopFile(desktopFile: string): Promise<void> {
  let info;
  try {
    info = await lstat(desktopFile);
  } catch (error) {
    if (isMissing(error)) return;
    throw error;
  }
  if (info.isSymbolicLink() || !info.isFile()) {
    throw new Error(
      `${desktopFile} exists and is not a BoxBox desktop entry; refusing to replace it`,
    );
  }
  const content = await readFile(desktopFile, "utf8");
  if (!content.split("\n").some((line) => line.trim() === DESKTOP_MARKER)) {
    throw new Error(
      `${desktopFile} was not created by BoxBox; refusing to replace it`,
    );
  }
}

async function inspectWritableFile(path: string): Promise<void> {
  let info;
  try {
    info = await lstat(path);
  } catch (error) {
    if (isMissing(error)) return;
    throw error;
  }
  if (info.isSymbolicLink() || !info.isFile()) {
    throw new Error(`${path} exists and is not a regular file`);
  }
}

function renderDesktopEntry(launcher: string): string {
  const template = desktopTemplate;
  if (!template.includes("Exec=@EXEC@ %U")) {
    throw new Error("The desktop entry template is missing Exec=@EXEC@ %U");
  }
  if (
    !template.includes("MimeType=inode/directory;") ||
    !template.includes("x-scheme-handler/file")
  ) {
    throw new Error(
      "The desktop entry template does not register folder and file URL handling",
    );
  }
  return template.replaceAll("@EXEC@", quoteDesktopExecArgument(launcher));
}

async function replaceAppDirectory(
  stagedApp: string,
  appDirectory: string,
  dataHome: string,
  existing: boolean,
): Promise<void> {
  if (!existing) {
    await rename(stagedApp, appDirectory);
    return;
  }

  const backup = join(dataHome, `.boxbox-backup-${randomUUID()}`);
  await rename(appDirectory, backup);
  try {
    await rename(stagedApp, appDirectory);
  } catch (error) {
    try {
      await rename(backup, appDirectory);
    } catch (restoreError) {
      throw new AggregateError(
        [error, restoreError],
        `Could not replace ${appDirectory}; the previous install is at ${backup}`,
      );
    }
    throw error;
  }
  await rm(backup, { recursive: true, force: true }).catch(() => {
    // The new install is valid. A stale backup is safer than deleting
    // anything else in an unexpected failure mode.
  });
}

async function writeFileAtomic(
  path: string,
  content: string | Uint8Array,
): Promise<void> {
  const temporary = join(
    dirname(path),
    `.${randomUUID()}-${path.split(sep).at(-1) ?? "boxbox"}.tmp`,
  );
  try {
    await writeFile(temporary, content, { mode: 0o644 });
    await chmod(temporary, 0o644);
    await rename(temporary, path);
  } catch (error) {
    await rm(temporary, { force: true }).catch(() => {});
    throw error;
  }
}

function runXdgMime(
  command: readonly string[],
  env: Readonly<Record<string, string>>,
): void {
  const [program, ...args] = command;
  if (program === undefined) throw new Error("Missing xdg-mime command");
  const result = spawnSync(program, args, {
    env,
    encoding: "utf8",
    stdio: ["ignore", "ignore", "pipe"],
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    const detail = result.stderr.trim();
    throw new Error(
      `xdg-mime could not set BoxBox as the default (exit ${result.status})${detail ? `: ${detail}` : ""}`,
    );
  }
}

function isMissing(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    error.code === "ENOENT"
  );
}

function isManagedInstall(value: unknown): boolean {
  return (
    typeof value === "object" &&
    value !== null &&
    "schema" in value &&
    value.schema === 1 &&
    "identifier" in value &&
    value.identifier === APP_ID
  );
}

const USAGE = `Usage: bun desktop/scripts/install.ts [--source <bundle>] [--set-default]

Installs the stable BoxBox desktop bundle under $XDG_DATA_HOME/boxbox and
writes applications/io.boxbox.desktop plus a hicolor icon for this user.
Without --set-default, your folder and file:// handlers are left unchanged.`;

if (import.meta.main) {
  try {
    const options = parseInstallArguments(process.argv.slice(2));
    if (options.help) {
      console.log(USAGE);
    } else {
      const result = await installDesktopIntegration({
        source: options.source,
        setDefault: options.setDefault,
      });
      console.log(`Installed ${APP_NAME} at ${result.appDirectory}`);
      console.log(`Launcher: ${result.launcher}`);
      console.log(`Desktop entry: ${result.desktopFile}`);
      console.log(
        options.setDefault
          ? "BoxBox now handles folders and file:// URLs by default."
          : "Existing default handlers were not changed. Use --set-default to opt in.",
      );
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
