import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { startServer } from "../desktop/src/server";
import assert from "node:assert/strict";

const root = resolve(import.meta.dir, "..");
const version = (await readFile(join(root, "VERSION"), "utf8")).trim();
const directory = join(root, "dist/release");
const stage = await mkdtemp(join(root, "dist/release-check-"));
async function run(command: string[], env = process.env): Promise<void> {
  const child = Bun.spawn(command, {
    env,
    stdout: "inherit",
    stderr: "inherit",
  });
  assert.equal(await child.exited, 0, command[0]);
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
  for (const [arch, machine] of [
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
    const header = Buffer.from(
      await Bun.file(join(stage, serverName, "boxbox"))
        .slice(0, 64)
        .arrayBuffer(),
    );
    assert.equal(header.subarray(0, 4).toString(), "\x7fELF");
    assert.equal(header.readUInt16LE(18), machine, arch);
  }
  const name = `boxbox-desktop-v${version}-linux-amd64`;
  await run(["tar", "-xzf", join(directory, `${name}.tar.gz`), "-C", stage]);
  const home = join(stage, "home with spaces");
  await mkdir(home);
  const env = {
    ...process.env,
    HOME: home,
    XDG_DATA_HOME: join(home, "data"),
    XDG_CONFIG_HOME: join(home, "config"),
  };
  const install = join(stage, name, "install.sh");
  await run(["sh", install], env);
  const app = join(home, "data/boxbox");
  const desktop = await readFile(
    join(home, "data/applications/io.boxbox.desktop"),
    "utf8",
  );
  assert.ok(desktop.includes("X-BoxBox-Managed=true"));
  assert.ok(desktop.includes("home with spaces"));
  assert.equal(
    await Bun.file(join(home, "config/mimeapps.list")).exists(),
    false,
    "default handler remains unchanged",
  );
  const preferences = join(home, "data/io.boxbox.desktop/stable/desktop");
  await mkdir(preferences, { recursive: true });
  await writeFile(
    join(preferences, "preferences.json"),
    '{"wallpaperTint":true}',
  );
  await run(["sh", install], env);
  assert.equal(
    await readFile(join(preferences, "preferences.json"), "utf8"),
    '{"wallpaperTint":true}',
    "update preserves user settings",
  );
  const previousHome = process.env.HOME;
  process.env.HOME = home;
  const server = await startServer(
    join(app, "Resources/app/boxbox-server"),
    join(stage, "server-data"),
  );
  try {
    const bootstrap = await fetch(server.url, { redirect: "manual" });
    assert.equal(bootstrap.status, 303);
    const cookie = bootstrap.headers.get("set-cookie")?.split(";")[0];
    assert.ok(cookie);
    const response = await fetch(new URL("/api/v1/files", server.url), {
      headers: { Cookie: cookie },
    });
    assert.equal(response.status, 200);
    const page = await fetch(new URL("/", server.url), {
      headers: { Cookie: cookie },
    });
    assert.match(await page.text(), /name="boxbox-desktop"/);
  } finally {
    process.env.HOME = previousHome;
    await server.stop();
  }
  console.log(
    "PASS: all package checksums; standalone installer; default handler unchanged; update preserves preferences; installed backend authenticates",
  );
} finally {
  await rm(stage, { recursive: true, force: true });
}
