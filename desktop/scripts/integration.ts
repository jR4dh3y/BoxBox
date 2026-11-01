import assert from "node:assert/strict";
import {
  access,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { startServer } from "../src/server";

const fixture = await mkdtemp(join(tmpdir(), "boxbox-desktop-integration-"));
const originalHome = process.env.HOME;
const originalData = process.env.XDG_DATA_HOME;
const originalPath = process.env.PATH;
const xdgVariables = [
  "XDG_CONFIG_HOME",
  "XDG_CONFIG_DIRS",
  "XDG_DATA_DIRS",
  "XDG_CURRENT_DESKTOP",
];
const originalXDG = new Map(xdgVariables.map((key) => [key, process.env[key]]));
const dataHome = join(fixture, ".local/share");
const configHome = join(fixture, ".config");
const bin = join(fixture, "bin");
await mkdir(bin, { recursive: true });
await mkdir(join(dataHome, "applications"), { recursive: true });
await mkdir(configHome, { recursive: true });
const icon = join(dataHome, "fixture.svg");
await writeFile(
  icon,
  '<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="#369"/></svg>',
);
await writeFile(
  join(bin, "xdg-open"),
  '#!/bin/sh\nprintf "%s" "$1" > "$HOME/opened-path"\n',
  { mode: 0o700 },
);
await writeFile(
  join(bin, "fixture-viewer"),
  '#!/bin/sh\nprintf "%s" "$1" > "$HOME/chosen-app-path"\n',
  { mode: 0o700 },
);
await writeFile(
  join(dataHome, "applications/boxbox-fixture.desktop"),
  `[Desktop Entry]\nType=Application\nName=BoxBox Fixture Viewer\nExec=${join(bin, "fixture-viewer")} %f\nIcon=${icon}\n`,
);
await writeFile(
  join(dataHome, "applications/boxbox-old.desktop"),
  "[Desktop Entry]\nType=Application\nName=Old Viewer\nExec=/bin/true %f\nMimeType=text/plain;\n",
);
const desktopDefaults = join(configHome, "example-mimeapps.list");
const originalDefaults =
  "# Preserve this comment\n[Default Applications]\ntext/plain=boxbox-old.desktop;\nimage/png=boxbox-old.desktop;\n";
await writeFile(desktopDefaults, originalDefaults);
process.env.HOME = fixture;
process.env.XDG_DATA_HOME = dataHome;
process.env.XDG_CONFIG_HOME = configHome;
process.env.XDG_CONFIG_DIRS = join(fixture, "system-config");
process.env.XDG_DATA_DIRS = "/usr/local/share:/usr/share";
process.env.XDG_CURRENT_DESKTOP = "Example";
process.env.PATH = `${bin}:${originalPath ?? ""}`;
let server: Awaited<ReturnType<typeof startServer>> | undefined;

function trashID(value: unknown): string {
  assert.ok(typeof value === "object" && value !== null && "item" in value);
  const item = value.item;
  assert.ok(
    typeof item === "object" &&
      item !== null &&
      "id" in item &&
      typeof item.id === "string",
  );
  return item.id;
}

async function waitForFile(path: string): Promise<string> {
  for (let attempt = 0; attempt < 50; attempt++) {
    try {
      return await readFile(path, "utf8");
    } catch {
      await Bun.sleep(20);
    }
  }
  throw new Error(`Desktop launcher did not produce ${path}`);
}

try {
  server = await startServer(
    resolve(import.meta.dir, "../resources/boxbox-server"),
    join(fixture, ".state"),
  );
  const origin = new URL(server.url).origin;
  const bootstrap = await fetch(server.url, { redirect: "manual" });
  const cookie = bootstrap.headers.get("set-cookie")?.split(";")[0];
  assert.ok(cookie);
  const sessionHeaders = { Cookie: cookie };
  async function post(path: string, body: unknown) {
    return fetch(origin + "/api/v1/desktop/" + path, {
      method: "POST",
      headers: { ...sessionHeaders, "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
  }
  const file = join(fixture, "report..txt");
  await writeFile(file, "contents to preserve");
  await writeFile(join(fixture, "report%20.txt"), "literal percent name");
  await writeFile(join(fixture, "report .txt"), "different sibling");
  const clipboardResponse = await post("clipboard", {
    operation: "cut",
    paths: [`root${fixture}/report%20.txt`],
  });
  assert.equal(
    clipboardResponse.status,
    400,
    await clipboardResponse.clone().text(),
  );
  let response = await post("trash", { path: `root${fixture}/report..txt` });
  assert.equal(response.status, 201, await response.clone().text());
  const id = trashID(await response.json());
  await assert.rejects(access(file));
  await writeFile(file, "new occupant");
  response = await post("trash/restore", { id });
  assert.equal(response.status, 409);
  assert.equal(await readFile(file, "utf8"), "new occupant");
  await rm(file);
  assert.equal((await post("trash/restore", { id })).ok, true);
  assert.equal(await readFile(file, "utf8"), "contents to preserve");
  await symlink(file, join(fixture, "link.txt"));
  response = await post("trash", { path: `root${fixture}/link.txt` });
  assert.equal(response.ok, true, await response.clone().text());
  const linkID = trashID(await response.json());
  assert.equal(await readFile(file, "utf8"), "contents to preserve");
  assert.equal((await post("trash/restore", { id: linkID })).ok, true);
  assert.equal(
    await readFile(join(fixture, "link.txt"), "utf8"),
    "contents to preserve",
  );
  response = await post("open", { path: `root${fixture}/report..txt` });
  assert.equal(response.status, 200, await response.clone().text());
  assert.equal(await waitForFile(join(fixture, "opened-path")), file);
  response = await fetch(
    origin + `/api/v1/desktop/apps?path=root${fixture}/report..txt`,
    { headers: { Cookie: cookie } },
  );
  assert.equal(response.status, 200);
  assert.match(await response.text(), /BoxBox Fixture Viewer/);
  response = await post("open-with", {
    path: `root${fixture}/report..txt`,
    appId: "boxbox-fixture.desktop",
  });
  assert.equal(response.ok, true, await response.clone().text());
  assert.equal(await waitForFile(join(fixture, "chosen-app-path")), file);
  await assert.rejects(access(join(configHome, "mimeapps.list")));
  await rm(join(fixture, "chosen-app-path"));
  response = await fetch(
    origin + "/api/v1/desktop/apps/icon?appId=boxbox-fixture.desktop",
    { headers: sessionHeaders },
  );
  assert.equal(response.status, 200, await response.clone().text());
  assert.equal(response.headers.get("content-type"), "image/png");
  assert.deepEqual(
    new Uint8Array(await response.arrayBuffer()).slice(0, 8),
    new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]),
  );
  response = await post("apps/default", {
    path: `root${fixture}/report..txt`,
    appId: "boxbox-fixture.desktop",
  });
  assert.equal(response.status, 200, await response.clone().text());
  assert.match(
    await readFile(join(configHome, "mimeapps.list"), "utf8"),
    /text\/plain=boxbox-fixture.desktop/,
  );
  assert.equal(
    await readFile(desktopDefaults, "utf8"),
    originalDefaults.replace(
      "text/plain=boxbox-old.desktop;",
      "text/plain=boxbox-fixture.desktop;boxbox-old.desktop;",
    ),
  );
  const association = Bun.spawn(["gio", "mime", "text/plain"], {
    env: { ...process.env },
    stdout: "pipe",
    stderr: "pipe",
  });
  const nativeAssociation = await new Response(association.stdout).text();
  assert.equal(await association.exited, 0);
  assert.match(
    nativeAssociation.split("\n")[0] ?? "",
    /boxbox-fixture.desktop/,
  );
  response = await fetch(
    origin + `/api/v1/desktop/apps?path=root${fixture}/report..txt`,
    { headers: sessionHeaders },
  );
  const apps: unknown = await response.json();
  assert.ok(
    typeof apps === "object" &&
      apps !== null &&
      "recommended" in apps &&
      Array.isArray(apps.recommended),
  );
  assert.ok(
    apps.recommended.some(
      (app: unknown) =>
        typeof app === "object" &&
        app !== null &&
        "id" in app &&
        app.id === "boxbox-fixture.desktop" &&
        "isDefault" in app &&
        app.isDefault === true,
    ),
  );
  // Launch the isolated GIO default directly; a host desktop portal uses its own XDG config.
  const defaultOpen = Bun.spawn(
    [
      "python3",
      "-I",
      "-c",
      'import sys; from gi.repository import Gio; Gio.AppInfo.get_default_for_type("text/plain", False).launch([Gio.File.new_for_path(sys.argv[1])], None)',
      file,
    ],
    { env: { ...process.env }, stdout: "pipe", stderr: "pipe" },
  );
  assert.equal(
    await defaultOpen.exited,
    0,
    await new Response(defaultOpen.stderr).text(),
  );
  assert.equal(await waitForFile(join(fixture, "chosen-app-path")), file);
  response = await fetch(origin + "/api/v1/desktop/volumes", {
    headers: { Cookie: cookie },
  });
  assert.equal(response.status, 200);
  console.log(
    "Desktop integration passed: Trash/restore, no overwrite, symlink safety, dot filenames, default opener, chosen application, rasterized application icon, isolated GIO default association and refreshed default flag, volume listing.",
  );
} finally {
  await server?.stop();
  if (originalHome === undefined) delete process.env.HOME;
  else process.env.HOME = originalHome;
  if (originalData === undefined) delete process.env.XDG_DATA_HOME;
  else process.env.XDG_DATA_HOME = originalData;
  if (originalPath === undefined) delete process.env.PATH;
  else process.env.PATH = originalPath;
  for (const [key, value] of originalXDG) {
    if (value === undefined) delete process.env[key];
    else process.env[key] = value;
  }
  await rm(fixture, { recursive: true, force: true });
}
