import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { mkdtemp, mkdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";

const root = resolve(import.meta.dir, "..");
const version = (await readFile(join(root, "VERSION"), "utf8")).trim();
const directory = join(root, "dist/release");
const stage = await mkdtemp(join(root, "dist/release-check-"));
async function run(
  command: [string, ...string[]],
  env = process.env,
): Promise<void> {
  const child = Bun.spawn(command, {
    env,
    stdout: "inherit",
    stderr: "inherit",
  });
  assert.equal(await child.exited, 0, command[0]);
}
async function machine(file: string): Promise<number> {
  const header = Buffer.from(await Bun.file(file).slice(0, 64).arrayBuffer());
  assert.equal(header.subarray(0, 4).toString(), "\x7fELF", file);
  return header.readUInt16LE(18);
}
function freePort(): Promise<number> {
  return new Promise((done, fail) => {
    const probe = createServer().listen(0, "127.0.0.1", () => {
      const address = probe.address();
      probe.close(() =>
        typeof address === "object" && address
          ? done(address.port)
          : fail(new Error("No loopback port")),
      );
    });
  });
}
/**
 * An `Exec` value as a launcher reads it: the value's string escapes, then
 * double-quoted arguments with `\` escapes, with field codes dropped and `%%` as `%`.
 */
function execArgv(value: string): string[] {
  const text = value.replace(/\\(.)/g, (_, next: string) =>
    next === "s" ? " " : next === "n" ? "\n" : next === "t" ? "\t" : next === "r" ? "\r" : next,
  );
  const argv: string[] = [];
  let word: string | undefined;
  for (let index = 0; index < text.length; index++) {
    const char = text[index];
    if (char === '"') {
      word ??= "";
      for (index++; index < text.length && text[index] !== '"'; index++)
        word += text[index] === "\\" ? text[++index] : text[index];
    } else if (char === " ") {
      if (word !== undefined) argv.push(word);
      word = undefined;
    } else word = (word ?? "") + char;
  }
  if (word !== undefined) argv.push(word);
  return argv
    .filter((arg) => !/^%[fFuUick]$/.test(arg))
    .map((arg) => arg.replaceAll("%%", "%"));
}

/** Runs the standalone server in loopback dev mode and checks it serves the API and embedded UI. */
async function checkServer(binary: string, data: string): Promise<void> {
  const port = await freePort();
  const files = join(data, "files");
  await mkdir(files, { recursive: true });
  const config = join(data, "config.json");
  await writeFile(
    config,
    JSON.stringify({
      port,
      data_dir: data,
      mount_points: [{ name: "files", path: files, read_only: false }],
    }),
  );
  const env = Object.fromEntries(
    Object.entries(process.env).filter(
      ([key]) => !key.startsWith("BOXBOX_") && !key.startsWith("FM_"),
    ),
  );
  const server = Bun.spawn([binary, "--dev", "--config", config], {
    cwd: data,
    env,
    stdout: "inherit",
    stderr: "inherit",
  });
  try {
    const url = `http://127.0.0.1:${port}`;
    const deadline = Date.now() + 15_000;
    let response: Response | undefined;
    while (!response) {
      assert.equal(server.exitCode, null, "server exited during startup");
      assert.ok(Date.now() < deadline, "server did not start");
      response = await fetch(`${url}/api/v1/files`).catch(() => undefined);
      if (!response) await Bun.sleep(100);
    }
    assert.equal(response.status, 200);
    const page = await fetch(`${url}/`);
    assert.equal(page.status, 200);
    assert.match(await page.text(), /<html/i);
  } finally {
    server.kill("SIGTERM");
    await server.exited;
  }
}

try {
  const sums = (await readFile(join(directory, "SHA256SUMS"), "utf8"))
    .trim()
    .split("\n");
  assert.equal(sums.length, 3);
  for (const line of sums) {
    const [expected, file] = line.split("  ");
    assert.ok(expected && /^[a-f0-9]{64}$/.test(expected));
    assert.ok(
      file &&
        /^boxbox(?:-desktop)?-v[\w.-]+-linux-(?:amd64|arm64)\.tar\.gz$/.test(
          file,
        ),
    );
    const hash = createHash("sha256");
    for await (const chunk of createReadStream(join(directory, file)))
      hash.update(chunk);
    assert.equal(hash.digest("hex"), expected, file);
  }
  for (const [arch, expected] of [
    ["amd64", 62],
    ["arm64", 183],
  ] satisfies [string, number][]) {
    const serverName = `boxbox-v${version}-linux-${arch}`;
    await run([
      "tar",
      "-xzf",
      join(directory, `${serverName}.tar.gz`),
      "-C",
      stage,
    ]);
    assert.equal(await machine(join(stage, serverName, "boxbox")), expected, arch);
  }
  await checkServer(
    join(stage, `boxbox-v${version}-linux-amd64/boxbox`),
    join(stage, "server-data"),
  );

  const name = `boxbox-desktop-v${version}-linux-amd64`;
  await run(["tar", "-xzf", join(directory, `${name}.tar.gz`), "-C", stage]);
  assert.equal(await machine(join(stage, name, "boxbox-files")), 62);
  const home = join(stage, "home with spaces");
  await mkdir(home);
  const env = {
    ...process.env,
    HOME: home,
    XDG_DATA_HOME: join(home, "data"),
    XDG_CONFIG_HOME: join(home, "config"),
    XDG_BIN_HOME: join(home, "bin"),
  };
  const install = join(stage, name, "install.sh");
  await run(["sh", install], env);
  const binary = join(home, "bin/boxbox-files");
  assert.ok(((await stat(binary)).mode & 0o111) !== 0, "binary is executable");
  const entry = join(home, "data/applications/io.boxbox.desktop");
  const exec = (await readFile(entry, "utf8")).match(/^Exec=(.*)$/m)?.[1];
  assert.ok(exec, "the entry has an Exec line");
  const argv = execArgv(exec);
  assert.deepEqual(argv, [binary], "Exec names the installed binary, spaces included");
  // Launch what the menu would, headless: the profile summary of an empty trace.
  const trace = join(stage, "empty-trace.json");
  await writeFile(trace, "[\n");
  await run([binary, "--profile-summary", trace], env);
  assert.ok(
    await Bun.file(
      join(home, "data/icons/hicolor/256x256/apps/io.boxbox.desktop.png"),
    ).exists(),
  );
  assert.equal(
    await Bun.file(join(home, "config/mimeapps.list")).exists(),
    false,
    "default handler remains unchanged",
  );
  const settings = join(home, "config/boxbox-gpui/settings.json");
  await mkdir(join(home, "config/boxbox-gpui"), { recursive: true });
  await writeFile(settings, '{"kept":true}');
  await run(["sh", install], env);
  await run(["sh", install, "--uninstall"], env);
  assert.equal(await Bun.file(binary).exists(), false, "uninstall removes the binary");
  assert.equal(await Bun.file(entry).exists(), false, "uninstall removes the entry");
  assert.equal(
    await readFile(settings, "utf8"),
    '{"kept":true}',
    "update and uninstall keep user settings",
  );
  console.log(
    "PASS: all package checksums; server architectures; standalone server serves the API and embedded UI; desktop installs without sudo, its menu entry launches the binary, it leaves the default handler, and keeps settings across update and uninstall",
  );
} finally {
  await rm(stage, { recursive: true, force: true });
}
