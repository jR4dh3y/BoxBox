import { afterEach, describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import {
  chmod,
  mkdir,
  mkdtemp,
  readFile,
  readlink,
  readdir,
  rm,
  stat,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import {
  browseURL,
  commandLineArguments,
  fileURLPath,
  findLauncherArguments,
  targetArguments,
  parseLaunchTarget,
  virtualPath,
} from "./launch";
import {
  installDesktopIntegration,
  parseInstallArguments,
  quoteDesktopExecArgument,
} from "../scripts/install";

const temporaryRoots: string[] = [];

afterEach(async () => {
  await Promise.all(
    temporaryRoots.splice(0).map((root) =>
      rm(root, { recursive: true, force: true }),
    ),
  );
});

async function temporaryRoot(label: string): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), `boxbox-${label}-`));
  temporaryRoots.push(root);
  return root;
}

async function folder(root: string, name: string): Promise<string> {
  const path = join(root, name);
  await mkdir(path, { recursive: true });
  return path;
}

async function file(root: string, name: string, content = "hello"): Promise<string> {
  const path = join(root, name);
  await writeFile(path, content);
  return path;
}

describe("desktop launch targets", () => {
  test("opens a folder path, including spaces, as a root mount path", async () => {
    const root = await temporaryRoot("launch-folder");
    const directory = await folder(root, "My Projects");
    expect(parseLaunchTarget([directory], { cwd: "/" })).toEqual({
      path: `root${directory}`,
    });
    expect(parseLaunchTarget(["My Projects"], { cwd: root })).toEqual({
      path: `root${directory}`,
    });
  });

  test("decodes file URLs and reveals a file by opening its parent", async () => {
    const root = await temporaryRoot("launch-url");
    const directory = await folder(root, "Web Assets");
    const target = await file(directory, "résumé #1.txt");
    const url = pathToFileURL(target).href;
    expect(url).toContain("%20");
    expect(parseLaunchTarget([url])).toEqual({
      path: `root${directory}`,
      select: `root${target}`,
    });
    expect(fileURLPath(pathToFileURL(directory).href)).toBe(directory);
  });

  test("--select reveals a file without treating it as a directory", async () => {
    const root = await temporaryRoot("launch-select");
    const directory = await folder(root, "Reports");
    const target = await file(directory, "Q4 summary.txt");
    expect(parseLaunchTarget(["--select", target])).toEqual({
      path: `root${directory}`,
      select: `root${target}`,
    });
    expect(parseLaunchTarget([`--select=${target}`])).toEqual({
      path: `root${directory}`,
      select: `root${target}`,
    });
    expect(parseLaunchTarget(["--select", target, directory])).toEqual({
      path: `root${directory}`,
      select: `root${target}`,
    });
    expect(() => parseLaunchTarget(["--select", directory])).toThrow(
      "expects a file",
    );
    expect(() => parseLaunchTarget(["--select", target, root])).toThrow(
      "directly inside",
    );
  });

  test("a plain file argument browses its parent and selects it", async () => {
    const root = await temporaryRoot("launch-file");
    const directory = await folder(root, "Archive");
    const target = await file(directory, "notes.md");
    expect(parseLaunchTarget([target])).toEqual({
      path: `root${directory}`,
      select: `root${target}`,
    });
  });

  test("accepts the first path when xdg-open passes several", async () => {
    const root = await temporaryRoot("launch-many");
    const first = await folder(root, "First");
    const second = await folder(root, "Second");
    expect(parseLaunchTarget([first, second])).toEqual({
      path: `root${first}`,
    });
  });

  test("rejects unknown options, duplicate selectors and remote schemes", async () => {
    const root = await temporaryRoot("launch-reject");
    const directory = await folder(root, "Safe");
    expect(() => parseLaunchTarget(["--launch", directory])).toThrow(
      "Unsupported launch option",
    );
    expect(() =>
      parseLaunchTarget(["--select", directory, "--select", directory]),
    ).toThrow("only once");
    for (const value of [
      "https://example.test/files",
      "javascript:alert(1)",
      "smb://server/share",
      "file://remote.test/tmp/file",
      "file:///tmp/file?download=1",
      "file:///tmp/file#fragment",
      "file:///tmp/%ZZ",
      "file://relative",
    ]) {
      expect(() => parseLaunchTarget([value])).toThrow();
    }
    expect(() => parseLaunchTarget(["--", "-safe"])).toThrow("no such file");
  });

  test("the -- separator allows a leading dash in a relative path", async () => {
    const root = await temporaryRoot("launch-dash");
    const directory = await folder(root, "-safe");
    expect(parseLaunchTarget(["--", "-safe"], { cwd: root })).toEqual({
      path: `root${directory}`,
    });
  });

  test("returns no target for an ordinary desktop launch", () => {
    expect(parseLaunchTarget([])).toBeUndefined();
  });

  test("builds a browse URL that carries the path and selection", async () => {
    const root = await temporaryRoot("launch-url-build");
    const directory = await folder(root, "A B");
    const target = await file(directory, "x y.txt");
    const url = new URL(
      browseURL("http://127.0.0.1:4567/_desktop?token=ignored", {
        path: `root${directory}`,
        select: `root${target}`,
      }),
    );
    expect(url.origin).toBe("http://127.0.0.1:4567");
    expect(url.pathname).toBe("/browse");
    expect(url.searchParams.get("path")).toBe(`root${directory}`);
    expect(url.searchParams.get("select")).toBe(`root${target}`);
    expect(() =>
      browseURL("file:///tmp/app", { path: "root/tmp" }),
    ).toThrow("HTTP(S)");
  });

  test("turns a target back into absolute launch arguments", () => {
    expect(targetArguments({ path: "root" })).toEqual(["/"]);
    expect(targetArguments({ path: "root/home/ana" })).toEqual(["/home/ana"]);
    expect(
      targetArguments({ path: "root/home/ana", select: "root/home/ana/a b.txt" }),
    ).toEqual(["--select", "/home/ana/a b.txt"]);
  });

  test("maps the filesystem root to the root mount", () => {
    expect(virtualPath("/")).toBe("root");
    expect(virtualPath("/home/ana")).toBe("root/home/ana");
  });

  test("reads packaged launcher arguments from the Linux command line", () => {
    expect(
      commandLineArguments(
        "/opt/Box Box/bin/launcher\0--select\0/tmp/My File.txt\0file:///tmp/Other%20Folder\0",
      ),
    ).toEqual([
      "--select",
      "/tmp/My File.txt",
      "file:///tmp/Other%20Folder",
    ]);
    expect(commandLineArguments("/opt/BoxBox/bin/launcher\0")).toEqual([]);
    expect(commandLineArguments("")).toEqual([]);
  });

  test("recovers packaged arguments from the waiting launcher process", () => {
    const launcher = "/opt/Box Box/bin/launcher";
    const wrapper = {
      parentPid: 1,
      executable: "/usr/bin/gio",
      arguments: ["gio", "open", "/tmp/ignored"],
    };
    const processes = new Map([
      [
        40,
        {
          parentPid: 30,
          executable: "/opt/Box Box/bin/bun",
          arguments: ["bun", "/opt/Box Box/Resources/main.js"],
        },
      ],
      [
        30,
        {
          parentPid: 20,
          executable: launcher,
          arguments: [launcher, "/tmp/My Folder", "file:///tmp/Second"],
        },
      ],
      [20, wrapper],
    ]);
    const inspect = (pid: number) => processes.get(pid);

    expect(findLauncherArguments(launcher, 30, inspect)).toEqual([
      "/tmp/My Folder",
      "file:///tmp/Second",
    ]);
    expect(findLauncherArguments(launcher, 40, inspect)).toEqual([
      "/tmp/My Folder",
      "file:///tmp/Second",
    ]);
    expect(findLauncherArguments("/usr/bin/launcher", 40, inspect)).toBeUndefined();
    expect(findLauncherArguments(launcher, 99, inspect)).toBeUndefined();
  });

  test("walks past wrapper processes but stops at the launcher", () => {
    const launcher = "/opt/boxbox/bin/launcher";
    const processes = new Map([
      [10, { parentPid: 1, executable: "/usr/bin/boxbox-launcher", arguments: ["boxbox-launcher"] }],
      [20, { parentPid: 10, executable: launcher, arguments: [launcher, "/tmp/one"] }],
      [30, { parentPid: 20, executable: "/usr/bin/sh", arguments: ["sh", "-c", "boxbox"] }],
      [40, { parentPid: 30, executable: "/usr/bin/bun", arguments: ["bun", "main.js"] }],
    ]);
    expect(
      findLauncherArguments(launcher, 40, (pid) => processes.get(pid)),
    ).toEqual(["/tmp/one"]);
    expect(
      findLauncherArguments(launcher, 99, (pid) => processes.get(pid)),
    ).toBeUndefined();
  });

  test("reads a live launcher parent from /proc on Linux", async () => {
    if (process.platform !== "linux") return;
    const root = await temporaryRoot("launch-proc");
    const bin = await folder(root, "bin");
    await symlink(process.execPath, join(bin, "bun"));
    const launcher = join(bin, "launcher");
    await writeFile(
      launcher,
      `#!/bin/sh\n"${join(bin, "bun")}" "${join(bin, "probe.ts")}" "$@"\nstatus=$?\nexit $status\n`,
      { mode: 0o755 },
    );
    await chmod(launcher, 0o755);
    await writeFile(
      join(bin, "probe.ts"),
      `import { launcherArguments } from ${JSON.stringify(join(import.meta.dir, "launch.ts"))};\n` +
        `console.log(JSON.stringify(launcherArguments(true)));\n`,
    );

    const result = spawnSync(
      launcher,
      ["/tmp/My Folder", "file:///tmp/Other%20Folder"],
      { encoding: "utf8" },
    );
    expect(result.status).toBe(0);
    expect(JSON.parse(result.stdout.trim())).toEqual([
      "/tmp/My Folder",
      "file:///tmp/Other%20Folder",
    ]);
  });
});

async function createBundle(
  root: string,
  options: {
    channel?: string;
    launcherBody?: string;
    launcherMode?: number;
  } = {},
): Promise<string> {
  const bundle = join(root, "bundle source");
  await mkdir(join(bundle, "bin"), { recursive: true });
  await mkdir(join(bundle, "Resources", "app"), { recursive: true });
  const launcher = join(bundle, "bin", "launcher");
  const mode = options.launcherMode ?? 0o755;
  await writeFile(
    launcher,
    options.launcherBody ?? "#!/bin/sh\nexit 0\n",
    { mode },
  );
  await chmod(launcher, mode);
  await writeFile(
    join(bundle, "Resources", "app", "boxbox-server"),
    "#!/bin/sh\nexit 0\n",
    { mode: 0o755 },
  );
  await writeFile(
    join(bundle, "Resources", "metadata.json"),
    JSON.stringify({
      identifier: "io.boxbox.desktop",
      name: "BoxBox",
      channel: options.channel ?? "stable",
      hash: "test",
    }),
  );
  return bundle;
}

interface TestEnvironment {
  [name: string]: string;
  HOME: string;
  XDG_DATA_HOME: string;
  XDG_CONFIG_HOME: string;
  PATH: string;
}

interface TarFixtureEntry {
  readonly name: string;
  readonly content?: string;
  readonly mode?: number;
  readonly type?: "file" | "directory" | "symlink";
  readonly linkName?: string;
}

function createTar(entries: readonly TarFixtureEntry[]): Buffer {
  const blocks: Buffer[] = [];
  for (const entry of entries) {
    const type = entry.type ?? "file";
    const content = Buffer.from(type === "file" ? (entry.content ?? "") : "");
    const mode =
      entry.mode ?? (type === "directory" ? 0o755 : type === "symlink" ? 0o777 : 0o644);
    blocks.push(tarHeader(entry, mode, content.length));
    if (content.length > 0) {
      blocks.push(content);
      const padding = (512 - (content.length % 512)) % 512;
      if (padding > 0) blocks.push(Buffer.alloc(padding));
    }
  }
  blocks.push(Buffer.alloc(1024));
  return Buffer.concat(blocks);
}

function tarHeader(entry: TarFixtureEntry, mode: number, size: number): Buffer {
  const header = Buffer.alloc(512);
  header.write(entry.name, 0, 100, "ascii");
  writeTarOctal(header, 100, 8, mode);
  writeTarOctal(header, 108, 8, 0);
  writeTarOctal(header, 116, 8, 0);
  writeTarOctal(header, 124, 12, size);
  writeTarOctal(header, 136, 12, 0);
  header[156] =
    entry.type === "directory" ? 0x35 : entry.type === "symlink" ? 0x32 : 0x30;
  if (entry.linkName !== undefined) {
    header.write(entry.linkName, 157, 100, "ascii");
  }
  header.write("ustar", 257, 6, "ascii");
  header.write("00", 263, 2, "ascii");
  header.fill(0x20, 148, 156);
  let sum = 0;
  for (const byte of header) sum += byte;
  header.write(sum.toString(8).padStart(6, "0"), 148, 6, "ascii");
  header[154] = 0;
  header[155] = 0x20;
  return header;
}

function writeTarOctal(
  header: Buffer,
  offset: number,
  length: number,
  value: number,
): void {
  const text = value.toString(8).padStart(length - 1, "0");
  header.write(text, offset, text.length, "ascii");
  header[offset + length - 1] = 0;
}

function runtimeEntries(
  overrides: { launcher?: string; server?: string } = {},
): TarFixtureEntry[] {
  return [
    { name: "BoxBox/", type: "directory" },
    { name: "BoxBox/bin/", type: "directory" },
    {
      name: "BoxBox/bin/launcher",
      content: overrides.launcher ?? "runtime launcher\n",
      mode: 0o755,
    },
    { name: "BoxBox/bin/cef/", type: "directory" },
    { name: "BoxBox/bin/cef/libcef.so", content: "fake cef" },
    { name: "BoxBox/bin/libcef.so", type: "symlink", linkName: "cef/libcef.so" },
    { name: "BoxBox/Resources/", type: "directory" },
    { name: "BoxBox/Resources/app/", type: "directory" },
    {
      name: "BoxBox/Resources/app/boxbox-server",
      content: overrides.server ?? "#!/bin/sh\nexit 0\n",
      mode: 0o755,
    },
    {
      name: "BoxBox/Resources/version.json",
      content: JSON.stringify({
        identifier: "io.boxbox.desktop",
        name: "BoxBox",
        channel: "stable",
      }),
    },
  ];
}

async function createArchiveBundle(
  root: string,
  entries: readonly TarFixtureEntry[],
): Promise<string> {
  const bundle = join(root, "archive bundle");
  await mkdir(join(bundle, "bin"), { recursive: true });
  await mkdir(join(bundle, "Resources"), { recursive: true });
  await writeFile(
    join(bundle, "bin", "launcher"),
    "#!/bin/sh\n# self-extracting wrapper\nexit 0\n",
    { mode: 0o755 },
  );
  const hash = "feedface";
  await writeFile(
    join(bundle, "Resources", "metadata.json"),
    JSON.stringify({
      identifier: "io.boxbox.desktop",
      name: "BoxBox",
      channel: "stable",
      hash,
    }),
  );
  await writeFile(
    join(bundle, "Resources", `${hash}.tar.zst`),
    Bun.zstdCompressSync(createTar(entries)),
  );
  return bundle;
}

function installEnvironment(root: string): TestEnvironment {
  return {
    HOME: join(root, "home"),
    XDG_DATA_HOME: join(root, "data home"),
    XDG_CONFIG_HOME: join(root, "config home"),
    PATH: "/usr/bin:/bin",
  };
}

describe("desktop install integration", () => {
  test("installs the bundle, icon and quoted desktop entry into temp XDG dirs", async () => {
    const root = await temporaryRoot("install");
    const bundle = await createBundle(root, { launcherMode: 0o750 });
    const env = installEnvironment(root);
    const commands: (readonly string[])[] = [];
    const result = await installDesktopIntegration({
      source: bundle,
      env,
      runCommand: (command) => {
        commands.push(command);
      },
    });

    expect(result.appDirectory).toBe(join(env.XDG_DATA_HOME, "boxbox"));
    expect(result.launcher).toBe(
      join(env.XDG_DATA_HOME, "boxbox", "bin", "launcher"),
    );
    const launcher = await stat(result.launcher);
    expect(launcher.mode & 0o777).toBe(0o750);
    expect(
      JSON.parse(
        await readFile(
          join(result.appDirectory, ".boxbox-install.json"),
          "utf8",
        ),
      ),
    ).toEqual({ schema: 1, identifier: "io.boxbox.desktop" });

    const desktop = await readFile(result.desktopFile, "utf8");
    expect(desktop).toContain(
      `Exec=${quoteDesktopExecArgument(result.launcher)} %U`,
    );
    expect(desktop).toContain("MimeType=inode/directory;x-scheme-handler/file;");
    expect(desktop).toContain("Icon=io.boxbox.desktop");
    expect(desktop).toContain("X-BoxBox-Managed=true");
    expect(desktop).not.toContain("@EXEC@");
    expect(result.launcher).toContain("data home");
    expect((await stat(result.iconFile)).size).toBeGreaterThan(0);
    expect(commands).toEqual([]);
  });

  test("accepts an already-expanded runtime with version identity", async () => {
    const root = await temporaryRoot("install-expanded");
    const bundle = await createBundle(root);
    await rm(join(bundle, "Resources", "metadata.json"));
    await writeFile(
      join(bundle, "Resources", "version.json"),
      JSON.stringify({
        identifier: "io.boxbox.desktop",
        name: "BoxBox",
        channel: "stable",
      }),
    );

    const result = await installDesktopIntegration({
      source: bundle,
      env: installEnvironment(root),
    });
    expect((await stat(result.launcher)).mode & 0o111).not.toBe(0);
    expect(
      await Bun.file(
        join(result.appDirectory, "Resources", "app", "boxbox-server"),
      ).exists(),
    ).toBe(true);
  });

  test("extracts the runtime payload from a self-extracting stable bundle", async () => {
    const root = await temporaryRoot("install-archive");
    const bundle = await createArchiveBundle(root, runtimeEntries());
    const env = installEnvironment(root);
    const commands: (readonly string[])[] = [];
    const result = await installDesktopIntegration({
      source: bundle,
      env,
      runCommand: (command) => {
        commands.push(command);
      },
    });

    expect(await readFile(result.launcher, "utf8")).toBe("runtime launcher\n");
    expect((await stat(result.launcher)).mode & 0o111).not.toBe(0);
    expect(await readlink(join(result.appDirectory, "bin", "libcef.so"))).toBe(
      "cef/libcef.so",
    );
    const server = join(
      result.appDirectory,
      "Resources",
      "app",
      "boxbox-server",
    );
    expect(await readFile(server, "utf8")).toContain("exit 0");
    expect((await stat(server)).mode & 0o111).not.toBe(0);
    const desktop = await readFile(result.desktopFile, "utf8");
    expect(desktop).toContain(
      `Exec=${quoteDesktopExecArgument(result.launcher)} %U`,
    );
    expect(desktop).not.toContain("self-extracting wrapper");
    expect(commands).toEqual([]);
    expect(existsSync(join(env.HOME, ".local", "share", "io.boxbox.desktop"))).toBe(
      false,
    );
  });

  test("refuses an unsafe archive without installing it", async () => {
    const root = await temporaryRoot("install-archive-unsafe");
    const env = installEnvironment(root);
    const bundle = await createArchiveBundle(join(root, "case"), [
      ...runtimeEntries(),
      { name: "BoxBox/../escape.txt", content: "no" },
    ]);

    await expect(
      installDesktopIntegration({ source: bundle, env }),
    ).rejects.toThrow(/Unsafe archive/);
    expect(existsSync(join(env.XDG_DATA_HOME, "boxbox"))).toBe(false);
    expect(existsSync(join(env.XDG_DATA_HOME, "escape.txt"))).toBe(false);
    expect(existsSync(join(root, "escape.txt"))).toBe(false);
  });

  test("replaces a previous managed install without leaving backups", async () => {
    const root = await temporaryRoot("install-upgrade");
    const env = installEnvironment(root);
    const firstBundle = await createBundle(root, {
      launcherBody: "#!/bin/sh\n# first\n",
    });
    const first = await installDesktopIntegration({
      source: firstBundle,
      env,
    });
    const secondBundle = await createBundle(join(root, "second"), {
      launcherBody: "#!/bin/sh\n# second\n",
    });
    const second = await installDesktopIntegration({
      source: secondBundle,
      env,
    });

    expect(second.appDirectory).toBe(first.appDirectory);
    expect(await readFile(second.launcher, "utf8")).toContain("second");
    const leftovers = (await readdir(env.XDG_DATA_HOME)).filter((name) =>
      name.startsWith(".boxbox-"),
    );
    expect(leftovers).toEqual([]);
  });

  test("refuses to replace an app directory or desktop entry it did not create", async () => {
    const root = await temporaryRoot("install-foreign");
    const bundle = await createBundle(root);
    const env = installEnvironment(root);
    const foreignApp = join(env.XDG_DATA_HOME, "boxbox");
    await mkdir(foreignApp, { recursive: true });
    const precious = join(foreignApp, "precious.txt");
    await writeFile(precious, "keep me");

    await expect(
      installDesktopIntegration({ source: bundle, env }),
    ).rejects.toThrow("not installed by BoxBox");
    expect(await readFile(precious, "utf8")).toBe("keep me");

    await rm(foreignApp, { recursive: true, force: true });
    const desktopFile = join(
      env.XDG_DATA_HOME,
      "applications",
      "io.boxbox.desktop",
    );
    await mkdir(join(env.XDG_DATA_HOME, "applications"), { recursive: true });
    await writeFile(desktopFile, "[Desktop Entry]\nName=Custom\n");

    await expect(
      installDesktopIntegration({ source: bundle, env }),
    ).rejects.toThrow("not created by BoxBox");
    expect(await readFile(desktopFile, "utf8")).toContain("Name=Custom");
    expect(existsSync(join(env.XDG_DATA_HOME, "boxbox"))).toBe(false);
  });

  test("rejects non-stable or incomplete bundles", async () => {
    const root = await temporaryRoot("install-bad-bundle");
    const env = installEnvironment(root);
    const devBundle = await createBundle(root, { channel: "dev" });
    await expect(
      installDesktopIntegration({ source: devBundle, env }),
    ).rejects.toThrow("stable");
    await expect(
      installDesktopIntegration({ source: join(root, "missing"), env }),
    ).rejects.toThrow("Not a stable BoxBox desktop bundle");
  });

  test("--set-default is explicit and invokes xdg-mime with isolated XDG dirs", async () => {
    const root = await temporaryRoot("install-default");
    const bundle = await createBundle(root);
    const env = installEnvironment(root);
    const bin = await folder(root, "bin");
    const capture = join(env.XDG_DATA_HOME, "xdg-mime-args");
    const configCapture = join(env.XDG_DATA_HOME, "xdg-mime-config");
    const script = join(bin, "xdg-mime");
    await writeFile(
      script,
      `#!/bin/sh\nprintf '%s\\n' "$@" > "${capture}"\nprintf '%s' "$XDG_CONFIG_HOME" > "${configCapture}"\n`,
      { mode: 0o755 },
    );
    await chmod(script, 0o755);
    env.PATH = `${bin}:/usr/bin:/bin`;

    await installDesktopIntegration({ source: bundle, env });
    expect(await Bun.file(capture).exists()).toBe(false);

    await installDesktopIntegration({ source: bundle, env, setDefault: true });
    expect((await readFile(capture, "utf8")).trim().split("\n")).toEqual([
      "default",
      "io.boxbox.desktop",
      "inode/directory",
      "x-scheme-handler/file",
    ]);
    expect(await readFile(configCapture, "utf8")).toBe(env.XDG_CONFIG_HOME);
  });

  test("the CLI installs the bundle with the same isolated environment", async () => {
    const root = await temporaryRoot("install-cli");
    const bundle = await createBundle(root);
    const env = installEnvironment(root);
    const script = join(import.meta.dir, "../scripts/install.ts");
    const result = spawnSync(
      process.execPath,
      [script, "--source", bundle],
      { env, encoding: "utf8" },
    );
    expect(result.status).toBe(0);
    expect(result.stdout).toContain("Installed BoxBox");
    expect(result.stdout).toContain("Existing default handlers were not changed");
    expect(
      (await stat(join(env.XDG_DATA_HOME, "applications", "io.boxbox.desktop")))
        .isFile(),
    ).toBe(true);
  });

  test("quotes Exec paths safely and parses installer options", () => {
    const quoted = quoteDesktopExecArgument(
      "/tmp/Box Box/$HOME/`whoami`/\"quote\"/back\\slash/100%/launcher",
    );
    expect(quoted.startsWith('"')).toBe(true);
    expect(quoted.endsWith('"')).toBe(true);
    expect(quoted).toContain("\\$HOME");
    expect(quoted).toContain("\\`whoami\\`");
    expect(quoted).toContain('\\"quote\\"');
    expect(quoted).toContain("back\\\\slash");
    expect(quoted).toContain("100%%");
    expect(() => quoteDesktopExecArgument("line\nbreak")).toThrow(
      "control characters",
    );
    expect(parseInstallArguments(["--source", "/tmp/app", "--set-default"])).toEqual(
      { source: "/tmp/app", setDefault: true, help: false },
    );
    expect(parseInstallArguments(["--help"])).toEqual({
      source: undefined,
      setDefault: false,
      help: true,
    });
    expect(() => parseInstallArguments(["--unknown"])).toThrow(
      "Unknown install option",
    );
  });
});
