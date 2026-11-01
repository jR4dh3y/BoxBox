import { expect, test } from "bun:test";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { readHardwareAcceleration, configureRendering } from "./rendering";

test("rendering choice follows the persisted app/channel including installed-root overrides", async () => {
  const root = await mkdtemp(join(tmpdir(), "boxbox-rendering-"));
  try {
    const resources = join(root, "Resources");
    await mkdir(resources);
    await writeFile(join(resources, "version.json"), JSON.stringify({ identifier: "io.boxbox.desktop", channel: "dev" }));
    const env = { XDG_DATA_HOME: root };
    const build = join(resources, "build.json");
    await writeFile(build, JSON.stringify({ chromiumFlags: { "disable-gpu": false, "disable-gpu-compositing": false, "remote-debugging-port": "9340" } }));
    const directory = join(root, "io.boxbox.desktop/dev/desktop");
    await mkdir(directory, { recursive: true });
    expect(await readHardwareAcceleration(resources, env)).toBe(true);
    await writeFile(join(directory, "preferences.json"), JSON.stringify({ hardwareAcceleration: false }));
    expect(await readHardwareAcceleration(resources, env)).toBe(false);
    expect(await configureRendering(resources, env)).toBe(false);
    expect(JSON.parse(await Bun.file(build).text()).chromiumFlags).toEqual({ "disable-gpu": true, "disable-gpu-compositing": true, "remote-debugging-port": "9340" });
    expect(await readHardwareAcceleration(resources, { ...env, ELECTROBUN_INSTALL_ROOT_NAME: "stable" })).toBe(true);
    await writeFile(join(directory, "preferences.json"), JSON.stringify({ hardwareAcceleration: true }));
    expect(await readHardwareAcceleration(resources, env)).toBe(true);
    expect(await configureRendering(resources, env)).toBe(true);
    expect(JSON.parse(await Bun.file(build).text()).chromiumFlags["disable-gpu"]).toBe(false);
  } finally { await rm(root, { recursive: true, force: true }); }
});
